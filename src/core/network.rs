use hyper::{body::to_bytes, Body, Client, Request};
use hyper_tls::HttpsConnector;
use std::{
    fmt::Display,
    future::Future,
    io::{Cursor, Read, Write},
    net::TcpStream,
    time::SystemTime,
};

use crate::define_observer;

use super::{
    jce::{decode_wrapper, encode_wrapper, JceElement, JceError},
    tea::{decrypt, encrypt, TeaError},
};

// static DEFAULT_SERVER: (&'static str, u16) = "msfwifi.3g.qq.com";
// static DEFAULT_PORT: u16 = 8080;

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

#[derive(Debug)]
pub struct NetworkError(String);

impl NetworkError {
    pub fn illegal_data() -> Self {
        NetworkError("illegal data".to_string())
    }
}

impl From<JceError> for NetworkError {
    fn from(err: JceError) -> Self {
        Self(err.to_string())
    }
}

impl From<String> for NetworkError {
    fn from(err: String) -> Self {
        Self(err)
    }
}

impl From<&str> for NetworkError {
    fn from(err: &str) -> Self {
        Self(err.to_string())
    }
}

impl From<TeaError> for NetworkError {
    fn from(err: TeaError) -> Self {
        Self(err.to_string())
    }
}

impl From<hyper::Error> for NetworkError {
    fn from(err: hyper::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<hyper::http::Error> for NetworkError {
    fn from(err: hyper::http::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<tokio::io::Error> for NetworkError {
    fn from(err: tokio::io::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<tokio::task::JoinError> for NetworkError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self(err.to_string())
    }
}

impl Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

define_observer!(
    NetworkObserver,
    (connected, ConnectedListeners, ()),
    (closed, ClosedListeners, ()),
    (error, ErrorListeners, (msg: &str)),
    (packet, PacketListeners, (buf: &[u8]))
);

#[derive(Debug, Clone, Copy)]
pub enum NetworkState {
    Closed,
    Connecting,
    Connected,
}

pub struct Network {
    state: NetworkState,
    require_close: bool,
    server_last_update_time: Option<SystemTime>,
    server_list: Vec<(String, u16)>,
    auto_search: bool,
    observer: NetworkObserver,
}

impl Network {
    pub fn new() -> Self {
        Self {
            state: NetworkState::Closed,
            require_close: false,
            server_last_update_time: None,
            server_list: Vec::new(),
            auto_search: true,
            observer: NetworkObserver::new(),
        }
    }

    pub fn state(&self) -> NetworkState {
        self.state
    }

    pub fn observer(&mut self) -> &mut NetworkObserver {
        &mut self.observer
    }

    pub async fn connect(&mut self) -> Result<(), NetworkError> {
        if let NetworkState::Closed = self.state {
            self.state = NetworkState::Connecting;

            self.resolve().await?;

            // 使用异步尝试连接服务器
            let tcp = self.tcp_establish().await?;
            // 将 tcp 流转移到新线程并持续读取数据流
            let _ = self.tcp_reading(tcp);

            self.state = NetworkState::Connected;
            self.observer.connected.raise();

            Ok(())
        } else {
            Err(NetworkError::from("connecting or connected"))
        }
    }

    async fn tcp_establish(&mut self) -> Result<TcpStream, NetworkError> {
        let target_server = self
            .server_list
            .first()
            .unwrap_or(&("msfwifi.3g.qq.com".to_string(), 8080))
            .clone();
        Ok(tokio::spawn(async { TcpStream::connect(target_server) }).await??)
    }

    async fn tcp_reading(&mut self, mut tcp: TcpStream) {
        let mut buf = Vec::with_capacity(100);

        let mut ready_buf = Vec::<u8>::with_capacity(2000);
        loop {
            if self.require_close {
                self.state = NetworkState::Closed;
                self.observer.closed.raise();
                return;
            }

            match tcp.read(&mut buf) {
                Ok(len) => {
                    ready_buf.write_all(&buf[..len]).unwrap();

                    while ready_buf.len() > 4 {
                        let len = u32::from_be_bytes(ready_buf[..4].try_into().unwrap()) as usize;
                        if ready_buf.len() - 4 >= len {
                            let packet_buf =
                                ready_buf.splice(..4 + len, []).skip(4).collect::<Vec<_>>();

                            self.observer.packet.raise(&packet_buf);
                        } else {
                            break;
                        }
                    }
                }
                Err(err) => {
                    self.observer.error.raise(err.to_string().as_str());
                    self.state = NetworkState::Closed;
                    self.observer.closed.raise();
                    self.connect();
                    return;
                }
            }
        }
    }

    async fn resolve(&mut self) -> Result<(), NetworkError> {
        if !self.auto_search {
            return Ok(());
        }

        if let Some(duration) = self
            .server_last_update_time
            .and_then(|time| Some(time.elapsed().unwrap()))
        {
            if duration.as_secs() < 3600 {
                return Ok(());
            }
        }

        // NodeJS 代码中原作者说明了第一和第二个是网络状态最好的,所以做了筛选
        // 但是这里就不做筛选了,因为没有必要,上面的代码默认就是用第一个
        let server_list = update_server_list().await?;
        if !server_list.is_empty() {
            self.server_list = server_list;
            self.server_last_update_time = Some(SystemTime::now());
        }
        Ok(())
    }
}

async fn update_server_list() -> Result<Vec<(String, u16)>, NetworkError> {
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
            .ok_or(NetworkError::illegal_data())
            .and_then(|value| {
                if let JceElement::List(value) = value {
                    Ok(value)
                } else {
                    Err(NetworkError::illegal_data())
                }
            })
            .and_then(|value| {
                value.into_iter().try_for_each(|ele| {
                    if let JceElement::StructBegin(mut ele) = ele {
                        let address =
                            String::try_from(ele.remove(&1).ok_or(NetworkError::illegal_data())?)?;
                        let port =
                            i16::try_from(ele.remove(&2).ok_or(NetworkError::illegal_data())?)?
                                as u16;

                        list.push((address, port));
                        Ok(())
                    } else {
                        Err(NetworkError::illegal_data())
                    }
                })?;
                Ok(())
            })?;

        Ok(list)
    } else {
        Err(NetworkError::illegal_data())
    }
}
