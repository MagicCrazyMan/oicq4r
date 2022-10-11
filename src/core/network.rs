use flate2::Decompress;
use hyper::{body::to_bytes, Body, Client};
use hyper_tls::HttpsConnector;
use log::{debug, error, info, warn};
use pin_project_lite::pin_project;
use std::{
    borrow::BorrowMut,
    collections::HashMap,
    fmt::Display,
    future::Future,
    io::{Cursor, Read, Seek, SeekFrom, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll, Waker},
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::{
    broadcast::{self, Sender},
    Mutex, MutexGuard,
};

use crate::core::helper::BUF_16;

use super::{
    base_client::{DataCenter, Statistics},
    error::Error,
    io::ReadExt,
    jce::{decode_wrapper, encode_wrapper, JceElement},
    tea::{decrypt, encrypt},
};

#[derive(Debug)]
pub enum NetworkError {
    RequestTimeout,
    RequestUnexpectedLocked,
    AlreadyConnectedOrConnecting,
    NotConnected,
    UnknownFlag(u8),
    UnknownCompressedFlag(i32),
    UnsuccessfulRetcode(i32),
    InvalidatedData,
}

impl std::error::Error for NetworkError {}

impl Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::RequestTimeout => f.write_str("request timeout."),
            NetworkError::RequestUnexpectedLocked => f.write_str("request unexpected locked."),
            NetworkError::AlreadyConnectedOrConnecting => {
                f.write_str("already connected or connecting.")
            }
            NetworkError::NotConnected => f.write_str("not connected."),
            NetworkError::UnknownFlag(flag) => {
                f.write_fmt(format_args!("unknown packet flag: {}.", flag))
            }
            NetworkError::UnknownCompressedFlag(flag) => {
                f.write_fmt(format_args!("unknown compressed flag: {}.", flag))
            }
            NetworkError::UnsuccessfulRetcode(retcode) => {
                f.write_fmt(format_args!("unsuccessful retcode flag: {}.", retcode))
            }
            NetworkError::InvalidatedData => f.write_str("invalidated decompress data."),
        }
    }
}

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

pub trait Request {
    fn seq(&self) -> u32;

    fn command(&self) -> &str;

    fn payload(&self) -> &[u8];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginCommand {
    WtLoginLogin,
    WtLoginExchangeEmp,
    WtLoginTransEmp,
    StatSvcRegister,
    ClientCorrectTime,
}

impl AsRef<str> for LoginCommand {
    fn as_ref(&self) -> &str {
        match self {
            LoginCommand::WtLoginLogin => "wtlogin.login",
            LoginCommand::WtLoginExchangeEmp => "wtlogin.exchange_emp",
            LoginCommand::WtLoginTransEmp => "wtlogin.trans_emp",
            LoginCommand::StatSvcRegister => "StatSvc.register",
            LoginCommand::ClientCorrectTime => "Client.CorrectTime",
        }
    }
}

pub struct LoginRequest {
    seq: u32,
    command: LoginCommand,
    payload: Vec<u8>,
}

impl LoginRequest {
    pub fn new(seq: u32, command: LoginCommand, payload: Vec<u8>) -> Self {
        Self {
            seq,
            command,
            payload,
        }
    }
}

impl Request for LoginRequest {
    fn seq(&self) -> u32 {
        self.seq
    }

    fn command(&self) -> &str {
        self.command.as_ref()
    }

    fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniCommand {
    OidbSvc,
}

impl AsRef<str> for UniCommand {
    fn as_ref(&self) -> &str {
        match self {
            UniCommand::OidbSvc => "OidbSvc.0x480_9_IMCore",
        }
    }
}

pub struct UniRequest {
    seq: u32,
    command: UniCommand,
    payload: Vec<u8>,
}

impl UniRequest {
    pub fn new(seq: u32, command: UniCommand, payload: Vec<u8>) -> Self {
        Self {
            seq,
            command,
            payload,
        }
    }
}

impl Request for UniRequest {
    fn seq(&self) -> u32 {
        self.seq
    }

    fn command(&self) -> &str {
        self.command.as_ref()
    }

    fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Debug, Clone)]
pub struct SSO {
    pub command: String,
    pub seq: u32,
    pub body: Vec<u8>,
}

#[derive(Debug)]
struct Polling {
    body: Option<Vec<u8>>,
    waker: Option<Waker>,
}

impl Polling {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            body: None,
            waker: None,
        }))
    }
}

pin_project! {
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Response {
        timeout: Duration,
        start: Instant,
        polling: Arc<Mutex<Polling>>,
    }
}

impl Future for Response {
    type Output = Result<Vec<u8>, NetworkError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start.elapsed() >= self.timeout {
            Poll::Ready(Err(NetworkError::RequestTimeout))
        } else {
            loop {
                if let Ok(mut polling) = self.polling.try_lock() {
                    if let Some(body) = polling.body.take() {
                        return Poll::Ready(Ok(body));
                    } else {
                        polling.waker = Some(cx.waker().clone());

                        let waker = cx.waker().clone();
                        let timeout = self.timeout;
                        tokio::spawn(async move {
                            tokio::time::sleep(timeout).await;
                            waker.wake_by_ref();
                        });

                        return Poll::Pending;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkState {
    Closed,
    ///若不是主动关闭连接，该状态则会在 [`NetworkState::Closed`] 后触发
    Lost(Option<String>),
    Connecting,
    Connected,
}

#[derive(Debug)]
pub struct Network {
    data: Arc<Mutex<DataCenter>>,
    statistics: Arc<Mutex<Statistics>>,

    tcp_writer: Arc<Mutex<Option<TcpStream>>>,
    tcp_reader_handler: Option<std::thread::JoinHandle<()>>,
    state: Arc<Mutex<NetworkState>>,
    close_manually: Arc<Mutex<bool>>,
    polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Polling>>>>>,

    server_last_update_time: Option<SystemTime>,
    server_list: Vec<(String, u16)>,
    auto_search: bool,

    state_tx: broadcast::Sender<(NetworkState, Option<SocketAddr>)>,
    sso_tx: broadcast::Sender<SSO>,
}

impl Network {
    pub fn new(data: Arc<Mutex<DataCenter>>, statistics: Arc<Mutex<Statistics>>) -> Self {
        Self {
            data,
            statistics,

            tcp_writer: Arc::new(Mutex::new(None)),
            tcp_reader_handler: None,
            state: Arc::new(Mutex::new(NetworkState::Closed)),
            close_manually: Arc::new(Mutex::new(false)),
            polling_requests: Arc::new(Mutex::new(HashMap::new())),

            server_last_update_time: None,
            server_list: vec![],
            auto_search: true,

            state_tx: broadcast::channel(1).0,
            sso_tx: broadcast::channel(1).0,
        }
    }

    pub fn on_sso(&self) -> broadcast::Receiver<SSO> {
        self.sso_tx.subscribe()
    }

    pub fn on_state(&self) -> broadcast::Receiver<(NetworkState, Option<SocketAddr>)> {
        self.state_tx.subscribe()
    }

    pub async fn state(&self) -> MutexGuard<NetworkState> {
        self.state.lock().await
    }

    pub async fn connect(&mut self) -> Result<(), Error> {
        if NetworkState::Closed == *self.state.lock().await {
            *self.close_manually.lock().await = false;

            // 更新状态至 Connecting
            *self.state.lock().await = NetworkState::Connecting;
            let _ = self.state_tx.send((NetworkState::Connecting, None));
            info!("Connecting server...");

            // 使用尝试连接服务器
            let tcp = TcpStream::connect(self.resolve_target_sevrer().await?)?;
            let target_server = tcp.peer_addr().ok();

            // 保留 tcp 写入流
            *self.tcp_writer.lock().await = Some(tcp.try_clone().unwrap());
            // 使用读取流持续读取内容
            self.tcp_reader_handler = Some(self.describe_packets_receiving(tcp));

            // 等待完成初始化后更新状态至 Connected
            *self.state.lock().await = NetworkState::Connected;
            self.statistics.lock().await.remote_socket_addr = target_server;
            let _ = self.state_tx.send((NetworkState::Connected, target_server));
            info!(
                "Connected to server {} successfully",
                target_server
                    .and_then(|server| Some(server.to_string()))
                    .unwrap_or("unknown".to_string())
            );

            Ok(())
        } else {
            Err(Error::from(NetworkError::AlreadyConnectedOrConnecting))
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), Error> {
        if NetworkState::Connected == *self.state.lock().await {
            *self.close_manually.lock().await = true;

            if let Some(stream) = self.tcp_writer.lock().await.take() {
                let _ = stream.shutdown(Shutdown::Both);
            }
            self.tcp_reader_handler = None;

            // Closed 事件会等待异步线程结束后触发
            Ok(())
        } else {
            Err(Error::from(NetworkError::NotConnected))
        }
    }

    pub async fn write_request<B: Request>(&mut self, request: B) -> Result<(), Error> {
        if NetworkState::Connected == *self.state.lock().await {
            let mut writer = self.tcp_writer.lock().await;
            let writer = writer.borrow_mut().as_mut().unwrap();
            writer.write_all(request.payload())?;

            self.statistics.lock().await.sent_pkt_cnt += 1;

            debug!(
                "Packet write with command: {}, seq: {}, len: {}",
                request.command(),
                request.seq(),
                request.payload().len()
            );
            Ok(())
        } else {
            Err(Error::from(NetworkError::NotConnected))
        }
    }

    pub async fn send_request<B: Request>(
        &mut self,
        request: B,
        timeout: Duration,
    ) -> Result<Response, Error> {
        if NetworkState::Connected == *self.state.lock().await {
            let response = Response {
                timeout,
                start: Instant::now(),
                polling: Polling::new(),
            };
            self.polling_requests
                .lock()
                .await
                .insert(request.seq(), Arc::downgrade(&response.polling));

            let mut writer = self.tcp_writer.lock().await;
            let writer = writer.borrow_mut().as_mut().unwrap();
            writer.write_all(request.payload())?;

            self.statistics.lock().await.sent_pkt_cnt += 1;

            debug!(
                "Packet sent with command: {}, seq: {}, len: {}, timeout: {}s",
                request.command(),
                request.seq(),
                request.payload().len(),
                timeout.as_secs()
            );
            Ok(response)
        } else {
            Err(Error::from(NetworkError::NotConnected))
        }
    }

    async fn resolve_target_sevrer(&mut self) -> Result<(&str, u16), Error> {
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

    fn describe_packets_receiving(&mut self, mut reader: TcpStream) -> std::thread::JoinHandle<()> {
        let data = Arc::clone(&self.data);
        let statistics = Arc::clone(&self.statistics);
        let state = Arc::clone(&self.state);
        let polling_requests = Arc::clone(&self.polling_requests);
        let close_manually = Arc::clone(&self.close_manually);
        let state_tx = self.state_tx.clone();
        let sso_tx = self.sso_tx.clone();

        std::thread::spawn(move || {
            // 断开 tcp 之后 peer_addr 会返回 None，因此需要提前拿出来
            let remote_addr = reader.peer_addr().ok();

            let mut e = None;
            let mut buf = [0; 2048];
            let mut ready_buf = Vec::<u8>::with_capacity(buf.len() * 4);

            loop {
                match reader.read(&mut buf) {
                    Ok(len) => {
                        if len == 0 {
                            break;
                        }

                        ready_buf.write_all(&buf[..len]).unwrap();
                        while ready_buf.len() > 4 {
                            let len =
                                u32::from_be_bytes(ready_buf[..4].try_into().unwrap()) as usize;

                            if ready_buf.len() >= len {
                                let packet_buf =
                                    ready_buf.splice(..len, []).skip(4).collect::<Vec<_>>();

                                statistics.blocking_lock().recv_pkt_cnt += 1;

                                if let Err(err) = decode_packet(
                                    Arc::clone(&data),
                                    Arc::clone(&polling_requests),
                                    sso_tx.clone(),
                                    packet_buf,
                                ) {
                                    error!("Packet decode error: {} ", err);
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        e = Some(Error::from(err));
                        break;
                    }
                }
            }

            if !*close_manually.blocking_lock() {
                let msg = e.and_then(|e| Some(e.to_string()));

                *state.blocking_lock() = NetworkState::Lost(msg.clone());
                statistics.blocking_lock().lost_times += 1;
                let _ = state_tx.send((NetworkState::Lost(msg.clone()), remote_addr));
                error!(
                    "Connection lost: {}, reason: {}.",
                    remote_addr
                        .and_then(|server| Some(server.to_string()))
                        .unwrap_or("unknown".to_string()),
                    msg.unwrap_or("unknown".to_string())
                );
            }

            *state.blocking_lock() = NetworkState::Closed;
            statistics.blocking_lock().remote_socket_addr = None;
            let _ = state_tx.send((NetworkState::Closed, remote_addr));
            warn!(
                "Connection closed: {}.",
                remote_addr
                    .and_then(|server| Some(server.to_string()))
                    .unwrap_or("unknown".to_string())
            );
        })
    }
}

async fn update_server_list() -> Result<Vec<(String, u16)>, Error> {
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
            hyper::Request::builder()
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
            .ok_or(Error::from(NetworkError::InvalidatedData))
            .and_then(|value| {
                if let JceElement::List(value) = value {
                    Ok(value)
                } else {
                    Err(Error::from(NetworkError::InvalidatedData))
                }
            })
            .and_then(|value| {
                value.into_iter().try_for_each(|ele| {
                    if let JceElement::StructBegin(mut ele) = ele {
                        let address = String::try_from(
                            ele.remove(&1)
                                .ok_or(Error::from(NetworkError::InvalidatedData))?,
                        )?;
                        let port = i16::try_from(
                            ele.remove(&2)
                                .ok_or(Error::from(NetworkError::InvalidatedData))?,
                        )? as u16;

                        list.push((address, port));
                        Ok(())
                    } else {
                        Err(Error::from(NetworkError::InvalidatedData))
                    }
                })?;
                Ok(())
            })?;

        Ok(list)
    } else {
        Err(Error::from(NetworkError::InvalidatedData))
    }
}

#[inline]
fn decode_packet(
    data: Arc<Mutex<DataCenter>>,
    polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Polling>>>>>,
    sso_tx: Sender<SSO>,
    packet: Vec<u8>,
) -> Result<(), Error> {
    let flag = packet[4];
    let offset = u32::from_be_bytes(packet[6..10].try_into()?);
    let encrypted = packet[offset as usize + 6..].to_vec();

    let decrypted = match flag {
        0 => encrypted,
        1 => decrypt(&encrypted, &data.blocking_lock().sig.d2key)?,
        2 => decrypt(&encrypted, &BUF_16)?,
        _ => {
            // let _ = error_tx.send(InternalErrorKind::Token);
            return Err(Error::from(NetworkError::UnknownFlag(flag)));
        }
    };

    let sso = parse_sso(decrypted.as_slice())?;
    debug!(
        "Packet received with command {}, seq: {}, raw_len: {}, decoded_len: {}",
        &sso.command,
        sso.seq,
        packet.len(),
        sso.body.len()
    );

    let mut polling_requests = polling_requests.blocking_lock();
    if let Some(polling) = polling_requests.remove(&sso.seq).and_then(|p| p.upgrade()) {
        let mut polling = polling.blocking_lock();
        polling.body = Some(sso.body);

        if let Some(waker) = &polling.waker {
            waker.wake_by_ref();
        }
    } else {
        let _ = sso_tx.send(sso);
    }

    Ok(())
}

#[inline]
fn parse_sso(buf: &[u8]) -> Result<SSO, Error> {
    let mut z_decompress = Decompress::new(true);
    let mut reader = Cursor::new(buf);
    let head_len = reader.read_u32()? as usize;
    let seq = reader.read_u32()?;
    let retcode = reader.read_i32()?;

    if retcode != 0 {
        Err(Error::from(NetworkError::UnsuccessfulRetcode(retcode)))
    } else {
        let offset = reader.read_u32()? as i64;
        reader.seek(SeekFrom::Current(offset - 4))?;

        let len = reader.read_u32()? as usize;
        let command = String::from_utf8(reader.read_bytes(len - 4)?)?;

        let skip = reader.read_u32()? as i64;
        reader.seek(SeekFrom::Current(skip - 4))?;
        let flag = reader.read_i32()?;

        let body = match flag {
            0 => buf[head_len + 4..].to_vec(),
            1 => {
                let mut decompressed = Vec::with_capacity(buf.len() - head_len - 4 + 512);
                match z_decompress.decompress_vec(
                    &buf[head_len + 4..],
                    &mut decompressed,
                    flate2::FlushDecompress::Finish,
                ) {
                    Ok(status) => match status {
                        flate2::Status::Ok => decompressed,
                        _ => return Err(Error::from(NetworkError::InvalidatedData)),
                    },
                    Err(err) => return Err(Error::from(err)),
                }
            }
            8 => buf[head_len..].to_vec(),
            _ => {
                return Err(Error::from(NetworkError::UnknownCompressedFlag(flag)));
            }
        };

        Ok(SSO { seq, command, body })
    }
}
// #[cfg(test)]
// mod test {
//     use std::{sync::Arc, time::Duration};

//     use tokio::{sync::Mutex, task::JoinError};

//     use crate::{core::error::CommonError, init_logger};

//     use super::{Network, NetworkState};

//     #[tokio::test]
//     async fn test_resolve() -> Result<(), CommonError> {
//         let mut network = Network::new();
//         let (addr, port) = network.resolve_target_sevrer().await?;
//         let socket_addr = (addr.to_string(), port);

//         assert_ne!(network.server_list.len(), 0);
//         assert_eq!(
//             socket_addr,
//             network
//                 .server_list
//                 .first()
//                 .and_then(|(addr, port)| Some((addr.to_string(), *port)))
//                 .unwrap()
//         );
//         Ok(())
//     }

//     #[tokio::test]
//     async fn test() -> Result<(), JoinError> {
//         init_logger();

//         let network = Arc::new(Mutex::new(Network::new()));

//         let mut rx = network.lock().await.on_state();
//         let cloned = Arc::clone(&network);
//         let handler = tokio::spawn(async move {
//             while let Ok((state, _)) = rx.recv().await {
//                 match state {
//                     NetworkState::Closed => {
//                         println!("Closed");
//                         break;
//                     }
//                     NetworkState::Lost => {
//                         println!("Closed");
//                         break;
//                     }
//                     NetworkState::Connecting => {
//                         println!("Connecting");
//                     }
//                     NetworkState::Connected => {
//                         println!("Connected");
//                         let _ = cloned.lock().await.disconnect().await;
//                     }
//                 }
//             }
//         });

//         let _ = network.lock().await.connect().await;
//         tokio::time::sleep(Duration::from_secs(2)).await;
//         let _ = network.lock().await.disconnect().await;
//         let _ = handler.await;
//         Ok(())
//     }
// }
