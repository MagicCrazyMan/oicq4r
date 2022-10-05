use std::{
    collections::HashMap,
    fmt::Display,
    future::Future,
    io::{Read, Write},
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, SystemTime},
};

use flate2::Decompress;
use log::Level;
use tokio::{
    sync::{
        self,
        broadcast::{Receiver, Sender},
        Mutex, MutexGuard,
    },
    task::JoinHandle,
    time::Instant,
};

use crate::define_observer;

use super::{
    constant::{BUF_0, BUF_16, BUF_4},
    device::{FullDevice, Platform, ShortDevice, APK},
    ecdh::ECDH,
    error::CommonError,
    io::{ReadExt, WriteExt},
    network::{Network, NetworkState},
    tea::{self, decrypt},
    tlv::WriteTlvExt,
};

#[derive(Debug, Clone, Copy)]
pub enum QrcodeResult {
    OtherError = 0x00,
    Timeout = 0x11,
    WaitingForScan = 0x30,
    WaitingForConfirm = 0x35,
    Canceled = 0x36,
}

#[derive(Debug, Clone, Copy)]
pub struct BigData {
    pub ip: &'static str,
    pub port: u16,
    pub sig_session: [u8; 0],
    pub session_key: [u8; 0],
}

impl BigData {
    fn new() -> Self {
        Self {
            ip: "",
            port: 0,
            sig_session: BUF_0,
            session_key: BUF_0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SIG {
    pub seq: u32,
    pub session: [u8; 4],
    pub randkey: [u8; 16],
    pub tgtgt: [u8; 16],
    pub tgt: [u8; 0],
    pub skey: [u8; 0],
    pub d2: [u8; 0],
    pub d2key: [u8; 16],
    pub t104: [u8; 0],
    pub t174: [u8; 0],
    pub qrsig: [u8; 0],
    pub bigdata: BigData,
    pub hb480: Vec<u8>,
    pub emp_time: u128,
    pub time_diff: i64,
}

impl SIG {
    fn new(uin: u32) -> Self {
        let hb480 = {
            let mut buf = Vec::with_capacity(9);
            buf.write_u32(uin).unwrap();
            buf.write_i32(0x19e39).unwrap();
            buf
        };

        Self {
            seq: rand::random::<u32>() & 0xfff,
            session: rand::random::<[u8; 4]>(),
            randkey: rand::random::<[u8; 16]>(),
            tgtgt: rand::random::<[u8; 16]>(),
            tgt: BUF_0,
            skey: BUF_0,
            d2: BUF_0,
            d2key: BUF_16,
            t104: BUF_0,
            t174: BUF_0,
            qrsig: BUF_0,
            bigdata: BigData::new(),
            hb480,
            emp_time: 0,
            time_diff: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Statistics {
    pub start_time: SystemTime,
    pub lost_times: usize,
    pub recv_pkt_cnt: usize,
    pub sent_pkt_cnt: usize,
    pub lost_pkt_cnt: usize,
    pub recv_msg_cnt: usize,
    pub sent_msg_cnt: usize,
    pub msg_cnt_per_min: usize,
    pub remote_socket_addr: Option<SocketAddr>,
}

impl Statistics {
    fn new() -> Self {
        Self {
            start_time: SystemTime::now(),
            lost_times: 0,
            recv_pkt_cnt: 0,
            sent_pkt_cnt: 0,
            lost_pkt_cnt: 0,
            recv_msg_cnt: 0,
            sent_msg_cnt: 0,
            msg_cnt_per_min: 0,
            remote_socket_addr: None,
        }
    }
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

// impl Display for LoginCommand {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             LoginCommand::WtLoginLogin => f.write_str("wtlogin.login"),
//             LoginCommand::WtLoginExchangeEmp => f.write_str("wtlogin.exchange_emp"),
//             LoginCommand::WtLoginTransEmp => f.write_str("wtlogin.trans_emp"),
//             LoginCommand::StatSvcRegister => f.write_str("StatSvc.register"),
//             LoginCommand::ClientCorrectTime => f.write_str("Client.CorrectTime"),
//         }
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginCommandType {
    Type0 = 0,
    Type1 = 1,
    Type2 = 2,
}

#[derive(Debug, Clone)]
pub enum InternalErrorKind {
    Token,
}

impl std::error::Error for InternalErrorKind {}

impl Display for InternalErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

define_observer! {
    Observer,
    (internal_qrcode, QrcodeListener, (qrcode: &[u8; 16])),
    (internal_slider, SliderListener, (url: &str)),
    (internal_verify, VerifyListener, (url: &str, phone: &str)),
    (internal_error_token, ErrorTokenListener, ()),
    (internal_error_network, ErrorNetworkListener, (code: i64, error: &InternalErrorKind)),
    (internal_error_login, ErrorLoginListener, (code: i64, message: &str)),
    (internal_error_qrcode, ErrorQrcodeListener, (code: &QrcodeResult, message: &str)),
    (internal_online, OnlineListener, (token: &[u8], nickname: &str, gender: u8, age: u8)),
    (internal_token, TokenListener, (token: &[u8])),
    (internal_kickoff, KickoffListener, (reason: &str)),
    (internal_sso, SsoListener, (cmd: &str, payload: &[u8], seq: i64)),
    (internal_verbose, VerboseListener, (verbose: &str, level: Level))
}

#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Request {
    timeout: Duration,
    start: Instant,
    packet: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Future for Request {
    type Output = Result<Vec<u8>, CommonError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start.elapsed() >= self.timeout {
            Poll::Ready(Err(CommonError::new("timeout")))
        } else {
            match self.packet.try_lock() {
                Ok(mut packet) => match packet.take() {
                    Some(packet) => return Poll::Ready(Ok(packet)),
                    None => {}
                },
                Err(_) => {}
            };

            let waker = cx.waker().clone();
            let timeout = self.timeout;
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                waker.wake_by_ref();
            });

            Poll::Pending
        }
    }
}

#[derive(Debug)]
pub struct BaseClient {
    network: Network,
    statistics: Arc<Mutex<Statistics>>,

    uin: u32,
    apk: APK,
    device: FullDevice,
    sig: Arc<Mutex<SIG>>,
    ecdh: ECDH,

    registered: Arc<Mutex<bool>>,

    subscribed_handlers: Vec<JoinHandle<()>>,
    heartbeat_handler: Option<JoinHandle<()>>,
    polling_requests: Arc<Mutex<HashMap<u32, Arc<Mutex<Option<Vec<u8>>>>>>>,

    verbose_tx: Sender<(String, Level)>,
    error_tx: Sender<InternalErrorKind>,
    sso_tx: Sender<(u32, String, Vec<u8>)>,
}

impl BaseClient {
    pub fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        let mut instance = Self {
            network: Network::new(),
            statistics: Arc::new(Mutex::new(Statistics::new())),

            uin,
            apk: platform.metadata(),
            device: FullDevice::from(d.unwrap_or(ShortDevice::generate(uin))),
            sig: Arc::new(Mutex::new(SIG::new(uin))),
            ecdh: ECDH::new(),

            registered: Arc::new(Mutex::new(false)),

            subscribed_handlers: Vec::with_capacity(3),
            heartbeat_handler: None,
            polling_requests: Arc::new(Mutex::new(HashMap::new())),

            verbose_tx: sync::broadcast::channel(1).0,
            error_tx: sync::broadcast::channel(1).0,
            sso_tx: sync::broadcast::channel(1).0,
        };

        instance.describe_network_error();
        instance.describe_network_state();
        instance.describe_network_packet();

        instance
    }

    pub fn default(uin: u32) -> Self {
        Self::new(uin, Platform::Android, None)
    }
}

impl BaseClient {
    pub fn on_verbose(&self) -> Receiver<(String, Level)> {
        self.verbose_tx.subscribe()
    }

    pub fn on_error(&self) -> Receiver<InternalErrorKind> {
        self.error_tx.subscribe()
    }

    pub fn on_sso(&self) -> Receiver<(u32, String, Vec<u8>)> {
        self.sso_tx.subscribe()
    }

    pub async fn register(&mut self) {
        *self.registered.lock().await = false;
    }

    // pub async fn connect(&mut self) -> Result<(), CommonError> {
    //     self.network.connect().await
    // }

    // pub async fn disconnect(&mut self)  -> Result<(), CommonError> {
    //     self.network.disconnect().await
    // }
}

impl BaseClient {
    async fn heartbeat(&mut self) {
        // 如果存在一个已有的异步线程，首先关闭
        if let Some(handler) = self.heartbeat_handler.take() {
            handler.abort();
            let _ = handler.await;
        }

        let registered = Arc::clone(&self.registered);
        self.heartbeat_handler = Some(tokio::spawn(async move {
            while *registered.lock().await {
                todo!()
            }
        }));
    }

    async fn synchronize_time(&mut self) -> Result<(), CommonError> {
        let packet = self
            .build_login_packet(
                LoginCommand::ClientCorrectTime,
                BUF_4,
                Some(LoginCommandType::Type0),
            )
            .await?;

        let response_packet = self.send_request(packet, None).await?.await?;
        let response_packet = &mut response_packet.as_slice();
        // 此处忽略错误
        if let Ok(server_time) = response_packet.read_i32() {
            self.sig().await.time_diff = server_time as i64
                - SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs() as i64;
        }

        Ok(())
    }

    async fn build_login_packet<B>(
        &mut self,
        command: LoginCommand,
        body: B,
        r#type: Option<LoginCommandType>,
    ) -> Result<Vec<u8>, CommonError>
    where
        B: AsRef<[u8]>,
    {
        self.increase_seq().await;

        let _ = self.verbose_tx.send((
            format!("send: {} seq: {}", command.as_ref(), self.sig().await.seq),
            Level::Debug,
        ));

        let r#type = r#type.unwrap_or(LoginCommandType::Type2);
        let (uin, cmd_id, subid) = match command {
            LoginCommand::WtLoginTransEmp => (0, 0x812, Platform::watch().subid),
            _ => (self.uin(), 0x810, self.apk().subid),
        };

        let mut buf_1;
        let body = match r#type {
            LoginCommandType::Type2 => {
                let encrypted = tea::encrypt(&body, &self.ecdh.share_key)?;
                let mut buf_0 =
                    Vec::with_capacity(24 + self.ecdh.public_key.len() + encrypted.len());
                buf_0.write_u8(0x02)?;
                buf_0.write_u8(0x01)?;
                buf_0.write_all(&self.sig().await.randkey)?;
                buf_0.write_u16(0x131)?;
                buf_0.write_u16(0x01)?;
                buf_0.write_u16(0x01)?;
                buf_0.write_tlv(&self.ecdh.public_key)?;
                buf_0.write_bytes(encrypted)?;

                buf_1 = Vec::with_capacity(29 + buf_0.len());
                buf_1.write_u8(0x02)?;
                buf_1.write_u16(29 + buf_0.len() as u16)?;
                buf_1.write_u16(8081)?;
                buf_1.write_u16(cmd_id)?;
                buf_1.write_u16(1)?;
                buf_1.write_u32(uin)?;
                buf_1.write_u8(3)?;
                buf_1.write_u8(0x87)?;
                buf_1.write_u8(0)?;
                buf_1.write_u32(2)?;
                buf_1.write_u32(0)?;
                buf_1.write_u32(0)?;
                buf_1.write_bytes(&body)?;
                buf_1.write_u8(0x03)?;

                buf_1.as_slice()
            }
            _ => body.as_ref(),
        };

        let mut buf =
            Vec::with_capacity(54 + command.as_ref().len() + self.device().imei.as_str().len());
        buf.write_u32(self.sig().await.seq)?;
        buf.write_u32(subid)?;
        buf.write_u32(subid)?;
        buf.write_bytes([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        ])?;
        buf.write_bytes_with_length(self.sig().await.tgt)?;
        buf.write_bytes_with_length(command.as_ref())?;
        buf.write_bytes_with_length(self.sig().await.session)?;
        buf.write_bytes_with_length(self.device().imei.as_str())?;
        buf.write_u32(4)?;
        buf.write_u16(2)?;
        buf.write_u32(4)?;
        let mut sso = Vec::with_capacity(buf.len() + body.len() + 8);
        sso.write_bytes_with_length(buf)?;
        sso.write_bytes_with_length(body)?;

        let encrypted_sso = match r#type {
            LoginCommandType::Type1 => tea::encrypt(sso, &self.sig().await.d2key)?,
            LoginCommandType::Type2 => tea::encrypt(sso, &BUF_16)?,
            _ => sso,
        };

        let mut packet = Vec::with_capacity(100);
        packet.write_u32(0x0A)?;
        packet.write_u8(r#type as u8)?;
        packet.write_bytes_with_length(self.sig().await.d2)?;
        packet.write_u8(0)?;
        packet.write_bytes_with_length(uin.to_string())?;
        packet.write_bytes(encrypted_sso)?;
        let mut result = Vec::with_capacity(packet.len() + 4);
        result.write_bytes_with_length(packet)?;

        Ok(result)
    }

    async fn increase_seq(&mut self) {
        let mut sig = self.sig().await;
        let mut next = sig.seq + 1;
        if next >= 0x8000 {
            next = 0
        }

        sig.seq = next;
    }

    /// 超时默认为 5s，如果需要自定义超时，请使用 `send_request_with_timeout`
    async fn send_request<B>(
        &mut self,
        payload: B,
        timeout: Option<Duration>,
    ) -> Result<Request, CommonError>
    where
        B: AsRef<[u8]>,
    {
        // 此处 Connected 返回的 Error 可直接忽略
        let _ = self.network.connect().await;

        let request = Request {
            timeout: timeout.unwrap_or(Duration::from_secs(5)),
            start: Instant::now(),
            packet: Arc::new(Mutex::new(None)),
        };
        let packet = Arc::clone(&request.packet);
        self.polling_requests
            .lock()
            .await
            .insert(self.sig().await.seq, packet);

        self.network.send_bytes(payload.as_ref()).await?;

        self.statistics().await.sent_pkt_cnt += 1;

        Ok(request)
    }

    // async fn send_request_with_timeout(&mut self, payload: &[u8]) -> Result<Request, CommonError> {
    //     self.send_request(payload, Duration::from_secs(5)).await
    // }

    async fn send_login_request(&mut self, cmd: LoginCommand, body: &[u8]) {}

    fn parse_sso(
        buf: &[u8],
        z_decompress: &mut Decompress,
    ) -> Result<(u32, String, Vec<u8>), CommonError> {
        let head_len = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        let seq = u32::from_be_bytes(buf[4..8].try_into().unwrap());
        let retcode = i32::from_be_bytes(buf[8..12].try_into().unwrap());

        if retcode != 0 {
            Err(CommonError::new(format!(
                "unsuccessful retcode: {}",
                retcode
            )))
        } else {
            let mut offset = u32::from_be_bytes(buf[12..4].try_into().unwrap()) as usize + 12;
            let len = u32::from_be_bytes(buf[offset..offset + 4].try_into().unwrap()) as usize;
            let cmd =
                String::from_utf8(buf.get(offset + 4..offset + len).unwrap().to_vec()).unwrap();
            offset += len;
            let len = u32::from_be_bytes(buf[offset..offset + 4].try_into().unwrap()) as usize;
            offset += len;
            let flag = i32::from_be_bytes(buf[offset..offset + 4].try_into().unwrap());

            let payload = match flag {
                0 => buf[head_len + 4..].to_vec(),
                1 => {
                    let mut decompressed = Vec::with_capacity(buf[head_len + 4..].len() + 100);
                    match z_decompress.decompress_vec(
                        &buf[head_len + 4..],
                        &mut decompressed,
                        flate2::FlushDecompress::Finish,
                    ) {
                        Ok(status) => match status {
                            flate2::Status::Ok => decompressed,
                            flate2::Status::BufError => {
                                return Err(CommonError::new("decompress buf error"))
                            }
                            flate2::Status::StreamEnd => {
                                return Err(CommonError::new("decompress stream error"))
                            }
                        },
                        Err(err) => return Err(CommonError::from(err)),
                    }
                }
                8 => buf[head_len..].to_vec(),
                _ => {
                    return Err(CommonError::new(format!(
                        "unknown compressed flag: {}",
                        flag
                    )));
                }
            };

            Ok((seq, cmd, payload))
        }
    }
}

impl BaseClient {
    fn describe_network_error(&mut self) {
        let mut rx = self.network.on_error();
        let verbose_tx = self.verbose_tx.clone();
        self.subscribed_handlers.push(tokio::spawn(async move {
            while let Ok(err) = rx.recv().await {
                let _ = verbose_tx.send((err.to_string(), Level::Error));
            }
        }));
    }

    fn describe_network_state(&mut self) {
        let statistics = Arc::clone(&self.statistics);
        let mut rx = self.network.on_state();
        let verbose_tx = self.verbose_tx.clone();
        self.subscribed_handlers.push(tokio::spawn(async move {
            while let Ok((state, socket_addr)) = rx.recv().await {
                let _ = match state {
                    NetworkState::Closed => {
                        statistics.lock().await.remote_socket_addr = None;

                        verbose_tx.send((
                            format!(
                                "{} closed",
                                socket_addr
                                    .and_then(|socket_addr| Some(socket_addr.to_string()))
                                    .unwrap_or("unknown remote server".to_string())
                            ),
                            Level::Info,
                        ))
                    }
                    NetworkState::Lost => {
                        statistics.lock().await.lost_times += 1;

                        verbose_tx.send((
                            format!(
                                "{} lost",
                                socket_addr
                                    .and_then(|socket_addr| Some(socket_addr.to_string()))
                                    .unwrap_or("unknown remote server".to_string())
                            ),
                            Level::Error,
                        ))
                    }
                    NetworkState::Connecting => {
                        verbose_tx.send((format!("network connecting..."), Level::Info))
                    }
                    NetworkState::Connected => {
                        statistics.lock().await.remote_socket_addr = socket_addr.clone();

                        verbose_tx.send((
                            format!(
                                "{} connected",
                                socket_addr
                                    .and_then(|socket_addr| Some(socket_addr.to_string()))
                                    .unwrap_or("unknown remote server".to_string())
                            ),
                            Level::Info,
                        ))
                    }
                };
            }
        }));
    }

    fn describe_network_packet(&mut self) {
        let statistics = Arc::clone(&self.statistics);
        let sig = Arc::clone(&self.sig);
        let polling_requests = Arc::clone(&self.polling_requests);

        let mut rx = self.network.on_packet();
        let error_tx = self.error_tx.clone();
        let verbose_tx = self.verbose_tx.clone();
        let sso_tx = self.sso_tx.clone();
        self.subscribed_handlers.push(tokio::spawn(async move {
            let mut z_decompress = Decompress::new(true);
            while let Ok(packet) = rx.recv().await {
                statistics.lock().await.recv_pkt_cnt += 1;

                let flag = packet[4];
                let offset = u32::from_be_bytes(packet[6..10].try_into().unwrap());
                let encrypted = packet[offset as usize + 6..].to_vec();

                let decrypted = match flag {
                    0 => encrypted,
                    1 => match decrypt(&encrypted, &sig.lock().await.d2key) {
                        Ok(decrypted) => decrypted,
                        Err(err) => {
                            let _ = verbose_tx.send((
                                format!("tea decrypted error: {}", err.to_string()),
                                Level::Error,
                            ));
                            continue;
                        }
                    },
                    2 => match decrypt(&encrypted, &BUF_16) {
                        Ok(decrypted) => decrypted,
                        Err(err) => {
                            let _ = verbose_tx.send((
                                format!("tea decrypted error: {}", err.to_string()),
                                Level::Error,
                            ));
                            continue;
                        }
                    },
                    _ => {
                        let _ = error_tx.send(InternalErrorKind::Token);
                        let _ = verbose_tx.send((format!("unknown flag {}", flag), Level::Error));
                        continue;
                    }
                };

                match BaseClient::parse_sso(decrypted.as_slice(), &mut z_decompress) {
                    Ok(sso) => {
                        let _ = verbose_tx
                            .send((format!("recv: {} seq: {}", sso.0, sso.1), Level::Debug));

                        if let Some(packet) = polling_requests.lock().await.remove(&sso.0) {
                            *packet.lock().await = Some(sso.2)
                        } else {
                            let _ = sso_tx.send(sso);
                        }
                    }
                    Err(err) => {
                        let _ =
                            verbose_tx.send((format!("sso parsec error: {}", err), Level::Error));
                    }
                };
            }
        }));
    }
}

impl BaseClient {
    pub fn uin(&self) -> u32 {
        self.uin
    }

    pub fn device(&self) -> &FullDevice {
        &self.device
    }

    pub fn apk(&self) -> APK {
        self.apk
    }

    pub async fn sig(&self) -> MutexGuard<SIG> {
        self.sig.lock().await
    }

    pub async fn statistics(&self) -> MutexGuard<Statistics> {
        self.statistics.lock().await
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::BaseClient;

    #[tokio::test]
    async fn test() {
        let mut base_client = BaseClient::default(1313);

        let mut rx = base_client.on_verbose();
        let handler = tokio::spawn(async move {
            while let Ok((message, level)) = rx.recv().await {
                match level {
                    log::Level::Error => println!("Error: {}", message),
                    log::Level::Warn => println!("Warn: {}", message),
                    log::Level::Info => println!("Info: {}", message),
                    log::Level::Debug => println!("Debug: {}", message),
                    log::Level::Trace => println!("Trace: {}", message),
                }
            }
        });

        // let _ = base_client.connect().await;
        // tokio::time::sleep(Duration::from_secs(2)).await;
        // base_client.disconnect().await;
        // drop(base_client);
        // let _ = handler.await;
    }
}
