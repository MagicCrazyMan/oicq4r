use std::{
    fmt::Display,
    io::{self, Cursor},
    net::SocketAddr,
    sync::Arc,
    time::SystemTime, collections::HashMap, future::Future,
};

use flate2::Decompress;
use log::Level;
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::{
        self,
        broadcast::{Receiver, Sender},
        Mutex, MutexGuard,
    },
    task::JoinHandle,
};

use crate::define_observer;

use super::{
    constant::{BUF_0, BUF_16},
    device::{FullDevice, Platform, ShortDevice, APK},
    error::CommonError,
    network::{Network, NetworkState},
    tea::decrypt,
    writer::{write_i32, write_u32},
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
    pub time_diff: u128,
}

impl SIG {
    fn new(uin: u32) -> Self {
        let hb480 = {
            let mut buf = Vec::with_capacity(9);
            write_u32(&mut buf, uin).unwrap();
            write_i32(&mut buf, 0x19e39).unwrap();
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
pub struct BaseClient {
    uin: u32,
    apk: APK,
    device: FullDevice,
    sig: Arc<Mutex<SIG>>,
    statistics: Arc<Mutex<Statistics>>,
    network: Network,

    subscribed_handlers: Vec<JoinHandle<()>>,
    // packet_handlers: Arc<Mutex<HashMap<i32, Box<dyn Future<Output = Vec<u8>>>>>>,

    verbose_tx: Sender<(String, Level)>,
    error_tx: Sender<InternalErrorKind>,
    sso_tx: Sender<(i32, String, Vec<u8>)>,
}

impl BaseClient {
    pub fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        let mut instance = Self {
            uin,
            apk: platform.metadata(),
            device: FullDevice::from(d.unwrap_or(ShortDevice::generate(uin))),
            sig: Arc::new(Mutex::new(SIG::new(uin))),
            statistics: Arc::new(Mutex::new(Statistics::new())),
            network: Network::new(),

            subscribed_handlers: Vec::with_capacity(3),

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

    pub async fn connect(&mut self) -> Result<(), CommonError> {
        self.network.connect().await
    }

    pub async fn disconnect(&mut self) {
        self.network.disconnect().await
    }
}

impl BaseClient {
    fn parse_sso(
        buf: &[u8],
        z_decompress: &mut Decompress,
    ) -> Result<(i32, String, Vec<u8>), CommonError> {
        let head_len = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        let seq = i32::from_be_bytes(buf[4..8].try_into().unwrap());
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
        let mut rx = self.network.on_state();
        let verbose_tx = self.verbose_tx.clone();
        self.subscribed_handlers.push(tokio::spawn(async move {
            while let Ok((state, socket_addr)) = rx.recv().await {
                let _ = match state {
                    NetworkState::Closed => verbose_tx.send((
                        format!(
                            "{} closed",
                            socket_addr
                                .and_then(|socket_addr| Some(socket_addr.to_string()))
                                .unwrap_or("unknown remote server".to_string())
                        ),
                        Level::Warn,
                    )),
                    NetworkState::Connecting => {
                        verbose_tx.send((format!("network connecting..."), Level::Info))
                    }
                    NetworkState::Connected => verbose_tx.send((
                        format!(
                            "{} connected",
                            socket_addr
                                .and_then(|socket_addr| Some(socket_addr.to_string()))
                                .unwrap_or("unknown remote server".to_string())
                        ),
                        Level::Info,
                    )),
                };
            }
        }));
    }

    fn describe_network_packet(&mut self) {
        let statistics = Arc::clone(&self.statistics);
        let sig = Arc::clone(&self.sig);

        let mut rx = self.network.on_packet();
        let error_tx = self.error_tx.clone();
        let verbose_tx = self.verbose_tx.clone();
        let sso_tx = self.sso_tx.clone();
        self.subscribed_handlers.push(tokio::spawn(async move {
            let statistics = statistics;
            let sig = sig;

            let error_tx = error_tx;
            let verbose_tx = verbose_tx;
            let sso_tx = sso_tx;

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

                        // todo!()， 此处未完成
                        
                        let _ = sso_tx.send(sso);
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

        let _ = base_client.connect().await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        base_client.disconnect().await;
        drop(base_client);
        let _ = handler.await;
    }
}
