use hyper::{body::to_bytes, Body, Client, Request};
use hyper_tls::HttpsConnector;
use std::{
    borrow::BorrowMut,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{self, Mutex},
    task::JoinHandle,
};

use super::{
    error::CommonError,
    jce::{decode_wrapper, encode_wrapper, JceElement},
    tea::{decrypt, encrypt},
};

static DEFAULT_SERVER: (&'static str, u16) = ("msfwifi.3g.qq.com", 8080);

static UPDATE_SERVER_KEY: [u8; 16] = [
    0xf0, 0x44, 0x1f, 0x5f, 0xf4, 0x2d, 0xa5, 0x8f, 0xdc, 0xf7, 0x94, 0x9a, 0xba, 0x62, 0xd4, 0x11,
];
static UPDATE_SEVER_REQUEST: [(&str, [u8; 45]); 1] = [(
    "HttpServerListReq",
    [
        0x0a, 0x1c, 0x2c, 0x30, 0x01, 0x46, 0x05, 0x30, 0x30, 0x30, 0x30, 0x30, 0x50, 0x64, 0x62,
        0x20, 0x02, 0xf6, 0x1d, 0x76, 0x0f, 0x33, 0x35, 0x36, 0x32, 0x33, 0x35, 0x30, 0x38, 0x38,
        0x36, 0x33, 0x34, 0x31, 0x35, 0x31, 0x8c, 0x9c, 0xac, 0xbc, 0xcc, 0xdc, 0xe0, 0x01, 0x0b,
    ],
)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    Closed,
    ///若不是主动关闭连接，该状态则会在 [`NetworkState::Closed`] 后触发
    Lost,
    Connecting,
    Connected,
}

#[derive(Debug)]
pub struct Network {
    tcp_writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
    tcp_reader_handler: Option<JoinHandle<()>>,
    state: Arc<Mutex<NetworkState>>,
    close_manually: Arc<Mutex<bool>>,

    server_last_update_time: Option<SystemTime>,
    server_list: Vec<(String, u16)>,
    auto_search: bool,

    state_tx: sync::broadcast::Sender<(NetworkState, Option<SocketAddr>)>,
    error_tx: sync::broadcast::Sender<CommonError>,
    packet_tx: sync::broadcast::Sender<Vec<u8>>,
}

impl Network {
    pub fn new() -> Self {
        Self {
            tcp_writer: Arc::new(Mutex::new(None)),
            tcp_reader_handler: None,
            state: Arc::new(Mutex::new(NetworkState::Closed)),
            close_manually: Arc::new(Mutex::new(false)),

            server_last_update_time: None,
            server_list: vec![],
            auto_search: true,

            state_tx: sync::broadcast::channel(1).0,
            error_tx: sync::broadcast::channel(1).0,
            packet_tx: sync::broadcast::channel(1).0,
        }
    }

    pub fn on_error(&self) -> sync::broadcast::Receiver<CommonError> {
        self.error_tx.subscribe()
    }

    pub fn on_packet(&self) -> sync::broadcast::Receiver<Vec<u8>> {
        self.packet_tx.subscribe()
    }

    pub fn on_state(&self) -> sync::broadcast::Receiver<(NetworkState, Option<SocketAddr>)> {
        self.state_tx.subscribe()
    }

    pub async fn state(&self) -> NetworkState {
        *self.state.lock().await
    }

    pub async fn connect(&mut self) -> Result<(), CommonError> {
        let state = self.state().await;
        if NetworkState::Closed == state {
            *self.close_manually.lock().await = false;

            // 更新状态至 Connecting
            *self.state.lock().await = NetworkState::Connecting;
            self.state_tx.send((NetworkState::Connecting, None))?;

            // 使用尝试连接服务器
            let tcp = TcpStream::connect(self.resolve_target_sevrer().await?).await?;
            let target_server = tcp.peer_addr().ok();

            let (readable, writeable) = tcp.into_split();
            // 保留 tcp 写入流
            *self.tcp_writer.lock().await = Some(writeable);
            // 使用读取流持续读取内容
            self.tcp_reader_handler = Some(self.describe_packets_receiving(readable));

            // 更新状态至 Connected
            *self.state.lock().await = NetworkState::Connected;
            self.state_tx
                .send((NetworkState::Connected, target_server))?;

            Ok(())
        } else {
            Err(CommonError::from("connected"))
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), CommonError> {
        if NetworkState::Connected == self.state().await {
            *self.close_manually.lock().await = true;

            if let Some(mut stream) = self.tcp_writer.lock().await.take() {
                let _ = stream.shutdown().await;
            }
            self.tcp_reader_handler = None;

            // Closed 事件会等待异步线程结束后触发
            Ok(())
        } else {
            Err(CommonError::from("not connected"))
        }
    }

    pub async fn send_bytes<B: AsRef<[u8]>>(&mut self, bytes: B) -> Result<(), CommonError> {
        let bytes = bytes.as_ref();
        if NetworkState::Connected == self.state().await {
            let mut writer = self.tcp_writer.lock().await;
            let writer = writer.borrow_mut().as_mut().unwrap();
            writer.write_all(bytes).await?;
            Ok(())
        } else {
            Err(CommonError::new("not connected"))
        }
    }

    async fn resolve_target_sevrer(&mut self) -> Result<(&str, u16), CommonError> {
        if !self.auto_search {
            return Ok(DEFAULT_SERVER);
        }

        if self
            .server_last_update_time
            .and_then(|time| Some(time.elapsed().unwrap_or(Duration::from_secs(3600))))
            .unwrap_or(Duration::from_secs(3600))
            .as_secs()
            >= 3600
        {
            // NodeJS 代码中原作者说明了第一和第二个是网络状态最好的,所以做了筛选
            // 但是这里就不做筛选了,因为没有必要,上面的代码默认就是用第一个
            let server_list = update_server_list().await?;
            if !server_list.is_empty() {
                self.server_list = server_list;
                self.server_last_update_time = Some(SystemTime::now());
            }
        }

        let target_server = self
            .server_list
            .first()
            .and_then(|(addr, port)| Some((addr.as_str(), *port)))
            .unwrap_or(DEFAULT_SERVER);

        Ok(target_server)
    }

    fn describe_packets_receiving(&mut self, mut reader: OwnedReadHalf) -> JoinHandle<()> {
        let state = Arc::clone(&self.state);
        let close_manually = Arc::clone(&self.close_manually);
        let packet_tx = self.packet_tx.clone();
        let state_tx = self.state_tx.clone();
        let error_tx = self.error_tx.clone();

        tokio::spawn(async move {
            // 断开 tcp 之后 peer_addr 会返回 None，因此需要提前拿出来
            let remote_addr = reader.peer_addr().ok();

            let state = state;
            let packet_tx = packet_tx;
            let state_tx = state_tx;
            let error_tx = error_tx;

            let mut error = None;
            let mut buf = Vec::with_capacity(100);
            let mut ready_buf = Vec::<u8>::with_capacity(2000);

            loop {
                match reader.read(&mut buf).await {
                    Ok(len) => {
                        if len == 0 {
                            break;
                        }

                        ready_buf.write_all(&buf[..len]).await.unwrap();
                        while ready_buf.len() > 4 {
                            let len =
                                u32::from_be_bytes(ready_buf[..4].try_into().unwrap()) as usize;
                            if ready_buf.len() - 4 >= len {
                                let packet_buf =
                                    ready_buf.splice(..4 + len, []).skip(4).collect::<Vec<_>>();

                                match packet_tx.send(packet_buf) {
                                    Ok(_) => {}
                                    Err(err) => {
                                        error = Some(CommonError::from(err));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        error = Some(CommonError::from(err));
                        break;
                    }
                }
            }

            if let Some(err) = error {
                error_tx.send(err).unwrap();
            }

            *state.lock().await = NetworkState::Closed;
            state_tx.send((NetworkState::Closed, remote_addr)).unwrap();

            if !*close_manually.lock().await {
                *state.lock().await = NetworkState::Lost;
                state_tx.send((NetworkState::Lost, remote_addr)).unwrap();
            }
        })
    }
}

async fn update_server_list() -> Result<Vec<(String, u16)>, CommonError> {
    let request_body = encode_wrapper(
        UPDATE_SEVER_REQUEST,
        "ConfigHttp",
        "HttpServerListReq",
        None,
    )?;
    let mut request_payload = ((request_body.len() + 4) as u32).to_be_bytes().to_vec();
    request_payload.extend(request_body);

    let encrypted_payload = encrypt(request_payload, &UPDATE_SERVER_KEY)?;

    let client = Client::builder().build::<_, hyper::Body>(HttpsConnector::new());
    let response = client
        .request(
            Request::builder()
                .method("POST")
                .uri("https://configsvr.msf.3g.qq.com/configsvr/serverlist.jsp?mType=getssolist")
                .body(Body::from(encrypted_payload))?,
        )
        .await?;
    let response_body: Vec<u8> = to_bytes(response).await?.into();

    let decrypted = decrypt(&response_body, &UPDATE_SERVER_KEY)?;
    let nested = decode_wrapper(&mut &decrypted[4..])?;

    if let JceElement::StructBegin(mut value) = nested {
        let mut list = Vec::new();

        value
            .remove(&2)
            .ok_or(CommonError::illegal_data())
            .and_then(|value| {
                if let JceElement::List(value) = value {
                    Ok(value)
                } else {
                    Err(CommonError::illegal_data())
                }
            })
            .and_then(|value| {
                value.into_iter().try_for_each(|ele| {
                    if let JceElement::StructBegin(mut ele) = ele {
                        let address =
                            String::try_from(ele.remove(&1).ok_or(CommonError::illegal_data())?)?;
                        let port =
                            i16::try_from(ele.remove(&2).ok_or(CommonError::illegal_data())?)?
                                as u16;

                        list.push((address, port));
                        Ok(())
                    } else {
                        Err(CommonError::illegal_data())
                    }
                })?;
                Ok(())
            })?;

        Ok(list)
    } else {
        Err(CommonError::illegal_data())
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::{sync::Mutex, task::JoinError};

    use crate::core::error::CommonError;

    use super::{Network, NetworkState};

    #[tokio::test]
    async fn test_resolve() -> Result<(), CommonError> {
        let mut network = Network::new();
        let (addr, port) = network.resolve_target_sevrer().await?;
        let socket_addr = (addr.to_string(), port);

        assert_ne!(network.server_list.len(), 0);
        assert_eq!(
            socket_addr,
            network
                .server_list
                .first()
                .and_then(|(addr, port)| Some((addr.to_string(), *port)))
                .unwrap()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test() -> Result<(), JoinError> {
        let network = Arc::new(Mutex::new(Network::new()));

        let mut rx = network.lock().await.on_state();
        let cloned = Arc::clone(&network);
        let handler = tokio::spawn(async move {
            while let Ok((state, _)) = rx.recv().await {
                match state {
                    NetworkState::Closed => {
                        println!("Closed");
                        break;
                    }
                    NetworkState::Lost => {
                        println!("Closed");
                        break;
                    }
                    NetworkState::Connecting => {
                        println!("Connecting");
                    }
                    NetworkState::Connected => {
                        println!("Connected");
                        let _ = cloned.lock().await.disconnect().await;
                    }
                }
            }
        });

        let _ = network.lock().await.connect().await;
        // drop(network);
        let _ = handler.await;
        Ok(())
    }
}
