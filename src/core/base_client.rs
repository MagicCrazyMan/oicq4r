use std::{
    collections::HashMap,
    fmt::Display,
    io::{Cursor, Seek, SeekFrom},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use async_recursion::async_recursion;
use log::error;
use tokio::{
    sync::{
        self,
        broadcast::{self, Receiver, Sender},
        Mutex, MutexGuard,
    },
    task::JoinHandle,
};

use crate::{
    core::{network::LoginCommand, protobuf::ProtobufElement},
    error::Error,
};

use super::{
    device::{FullDevice, Platform, ShortDevice, APK},
    ecdh::ECDH,
    helper::{current_unix_timestamp_as_secs, BUF_0, BUF_16, BUF_4},
    io::{ReadExt, WriteExt},
    jce::{self, JceElement, JceError, JceObject},
    network::{LoginRequest, Network, NetworkState, Request, Response, UniRequest, SSO},
    protobuf::{self, ProtobufObject},
    tea,
    tlv::{self, ReadTlvExt, TlvError, WriteTlvExt},
};

#[derive(Debug)]
pub enum LoginResult {
    Succeeded(User),
    Failure(String, String),
    SliderAuthorizationRequired(String),
    SecurityAuthorizationRequired(Option<String>, Option<String>),
    SecurityCodeSent,
    SecurityCodeIncorrect,
    BadToken,
    QrcodeUinNotMatched(u32, u32),
    QrcodeTimeout,
    QrcodeNotScanned,
    QrcodeNotConfirmed,
    QrcodeCancelled,
    QrcodeServerError,
    UnknownLoginType(u8),
}

#[derive(Debug)]
pub enum ClientError {
    FetchQrcodeFailure,
    NotRegistered,
    HeartbeatFailure,
}

impl std::error::Error for ClientError {}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ClientError::FetchQrcodeFailure => f.write_str("fetch qrcode failure."),
            ClientError::NotRegistered => {
                f.write_str("tried to send registered request before registered.")
            }
            ClientError::HeartbeatFailure => f.write_str("heartbeat failure."),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BigData {
    pub socket_addr: Option<SocketAddrV4>,
    pub sig_session: Vec<u8>,
    pub session_key: [u8; 16],
}

impl BigData {
    fn new() -> Self {
        Self {
            socket_addr: None,
            sig_session: vec![],
            session_key: [0; 16],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SIG {
    pub seq: u32,
    pub session: [u8; 4],
    pub randkey: [u8; 16],
    pub tgtgt: [u8; 16],
    pub tgt: Vec<u8>,
    pub skey: Vec<u8>,
    pub d2: Vec<u8>,
    pub d2key: [u8; 16],
    pub t104: Vec<u8>,
    pub t174: Vec<u8>,
    pub qrsig: Option<Vec<u8>>,
    pub bigdata: BigData,
    pub hb480: Vec<u8>,
    pub emp_time: u64,
    pub time_diff: i64,
}

impl SIG {
    fn new(uin: u32) -> Self {
        let hb480 = {
            let mut buf = Vec::with_capacity(9);
            buf.write_u32(uin).unwrap();
            buf.write_u8(0).unwrap();
            buf.write_i32(0x19e39).unwrap();

            protobuf::encode(&ProtobufObject::from([
                (1, ProtobufElement::from(1152)),
                (2, ProtobufElement::from(9)),
                (4, ProtobufElement::from(buf)),
            ]))
            .unwrap()
        };

        Self {
            seq: rand::random::<u32>() & 0xfff,
            session: rand::random::<[u8; 4]>(),
            randkey: rand::random::<[u8; 16]>(),
            tgtgt: rand::random::<[u8; 16]>(),
            tgt: vec![],
            skey: vec![],
            d2: vec![],
            d2key: [0; 16],
            t104: vec![],
            t174: vec![],
            qrsig: None,
            bigdata: BigData::new(),
            hb480,
            emp_time: 0,
            time_diff: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Statistics {
    pub lost_pkt_cnt: usize,
    pub lost_times: usize,
    pub msg_cnt_per_min: usize,
    pub recv_msg_cnt: usize,
    pub recv_pkt_cnt: usize,
    pub remote_socket_addr: Option<SocketAddr>,
    pub sent_msg_cnt: usize,
    pub sent_pkt_cnt: usize,
    pub start_time: SystemTime,
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
pub enum CommandType {
    Type0 = 0,
    Type1 = 1,
    Type2 = 2,
}

#[derive(Debug)]
pub struct BaseClient {
    statistics: Arc<Mutex<Statistics>>,
    data: Arc<Mutex<DataCenter>>,
    networker: Arc<Mutex<Networker>>,
    registry: Arc<Mutex<Registry>>,

    kickoff_tx: broadcast::Sender<(String, String)>,
}

impl BaseClient {
    pub async fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        let data = Arc::new(Mutex::new(DataCenter::new(uin, platform, d)));
        let statistics = Arc::new(Mutex::new(Statistics::new()));
        let networker = Arc::new(Mutex::new(Networker::new(
            Arc::clone(&statistics),
            Arc::clone(&data),
        )));
        let registry = Arc::new(Mutex::new(Registry::new(
            Arc::clone(&data),
            Arc::clone(&networker),
        )));
        let mut instance = Self {
            data,
            networker,
            statistics,
            registry,

            kickoff_tx: broadcast::channel(1).0,
        };

        instance.describe_network_state().await;
        instance.describe_network_push().await;

        instance
    }

    pub async fn default(uin: u32) -> Self {
        Self::new(uin, Platform::Android, None).await
    }

    pub async fn connect(&self) -> Result<(), Error> {
        self.networker.lock().await.connect().await
    }

    pub async fn disconnect(&self) -> Result<(), Error> {
        self.networker.lock().await.disconnect().await
    }

    pub async fn data(&self) -> MutexGuard<DataCenter> {
        self.data.lock().await
    }

    pub async fn statistics(&self) -> MutexGuard<Statistics> {
        self.statistics.lock().await
    }

    pub async fn send_registered_request(
        &self,
        request: UniRequest,
        timeout: Option<Duration>,
    ) -> Result<Response, Error> {
        self.networker
            .lock()
            .await
            .send_registered_request(request, timeout)
            .await
    }

    pub async fn on_kickoff(&self) -> Receiver<(String, String)> {
        self.kickoff_tx.subscribe()
    }

    pub async fn on_token_update(&self) -> Receiver<Vec<u8>> {
        self.registry.lock().await.on_token_update()
    }
}

impl BaseClient {
    async fn describe_network_state(&mut self) {
        let networker = Arc::clone(&self.networker);
        let register = Arc::clone(&self.registry);

        let mut rx = self.networker.lock().await.on_state();
        tokio::spawn(async move {
            while let Ok((state, _)) = rx.recv().await {
                if let NetworkState::Lost(_) = state {
                    networker.lock().await.set_registered(false);

                    loop {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        if let Err(_) = networker.lock().await.connect().await {
                            continue;
                        }
                        if let Ok(_) = register.lock().await.register().await {
                            break;
                        }
                    }
                };
            }
        });
    }

    async fn describe_network_push(&mut self) {
        let data = Arc::clone(&self.data);
        let networker = Arc::clone(&self.networker);
        let register = Arc::clone(&self.registry);
        let kickoff_tx = self.kickoff_tx.clone();

        let mut rx = self.networker.lock().await.on_push();
        tokio::spawn(async move {
            while let Ok(sso) = rx.recv().await {
                if sso.command == "StatSvc.ReqMSFOffline"
                    || sso.command == "MessageSvc.PushForceOffline"
                {
                    if let Ok(nested) = jce::decode_wrapper(&mut sso.body.as_slice()) {
                        if let JceElement::StructBegin(mut obj) = nested {
                            let nest1 = obj.remove(&1);
                            let nest2 = obj.remove(&2);
                            let nest3 = obj.remove(&3);
                            let nest4 = obj.remove(&4);

                            let a = if let (Some(typee), Some(reason)) = (nest3, nest4) {
                                Some((typee, reason))
                            } else if let (Some(typee), Some(reason)) = (nest1, nest2) {
                                Some((typee, reason))
                            } else {
                                None
                            };

                            if let Some((typee, reason)) = a {
                                if let (Ok(typee), Ok(reason)) =
                                    (String::try_from(typee), String::try_from(reason))
                                {
                                    let _ = kickoff_tx.send((typee, reason));
                                    let _ = register.lock().await.unregister().await;
                                    continue;
                                }
                            }
                        }
                    }

                    error!(
                        "push event StatSvc.ReqMSFOffline or MessageSvc.PushForceOffline error."
                    );
                } else if sso.command == "QualityTest.PushList"
                    || sso.command == "OnlinePush.SidTicketExpired"
                {
                    if let Ok(request) =
                        data.lock()
                            .await
                            .build_uni_request(sso.command, sso.body, Some(sso.seq))
                    {
                        if let Ok(_) = networker
                            .lock()
                            .await
                            .write_registered_request(request)
                            .await
                        {
                            continue;
                        }
                    }

                    error!("push event QualityTest.PushList or OnlinePush.SidTicketExpired error.");
                } else if sso.command == "ConfigPushSvc.PushReq" {
                    let mut body = if sso.body[0] == 0 {
                        &sso.body[4..]
                    } else {
                        sso.body.as_slice()
                    };

                    let result = jce::decode_wrapper(&mut body)
                        .and_then(|nested| {
                            if let JceElement::StructBegin(obj) = nested {
                                Ok(obj)
                            } else {
                                Err(JceError::DecodeError)?
                            }
                        })
                        .and_then(|mut obj| {
                            let n1: i64 = obj.try_remove(&1)?.try_into()?;
                            if n1 == 2 {
                                let n2: Vec<u8> = obj.try_remove(&2)?.try_into()?;
                                Ok(n2)
                            } else {
                                Err(JceError::DecodeError)?
                            }
                        })
                        .and_then(|n2| jce::decode(&mut n2.as_slice()))
                        .and_then(|mut d| {
                            let e = d.try_remove(&5)?;
                            if let JceElement::StructBegin(obj) = e {
                                Ok(obj)
                            } else {
                                Err(JceError::DecodeError)?
                            }
                        })
                        .and_then(|mut obj| {
                            let buf: Vec<u8> = obj.try_remove(&5)?.try_into()?;
                            let mut decoded = protobuf::decode(&mut buf.as_slice())?;

                            let d1281: Vec<u8> = decoded.try_remove(&1281)?.try_into()?;
                            let mut d1281_decoded = protobuf::decode(&mut d1281.as_slice())?;

                            let n1: Vec<u8> = d1281_decoded.try_remove(&1)?.try_into()?;
                            let n2: Vec<u8> = d1281_decoded.try_remove(&2)?.try_into()?;
                            let n2: [u8; 16] = n2[..16].try_into()?;

                            let mut socket_addr = None;
                            let n3: Vec<ProtobufElement> =
                                d1281_decoded.try_remove(&3)?.try_into()?;
                            for e in n3 {
                                let mut v: ProtobufObject = e.try_into()?;

                                let i: isize = v.try_remove(&1)?.try_into()?;
                                if i == 10 {
                                    let mut l: Vec<ProtobufElement> =
                                        v.try_remove(&2)?.try_into()?;

                                    let mut d: ProtobufObject = l.remove(0).try_into()?;

                                    let ip_integer: isize = d.try_remove(&2)?.try_into()?;
                                    let ipv4 = Ipv4Addr::from(ip_integer as u32);
                                    let port: isize = d.try_remove(&3)?.try_into()?;

                                    socket_addr = Some(SocketAddrV4::new(ipv4, port as u16));
                                }
                            }

                            Ok((n1, n2, socket_addr))
                        });

                    let mut data = data.lock().await;
                    if let Ok((n1, n2, socket_addr)) = result {
                        data.sig.bigdata.sig_session = n1;
                        data.sig.bigdata.session_key = n2;
                        data.sig.bigdata.socket_addr = socket_addr;
                    }
                }

                error!("push event ConfigPushSvc.PushReq error.");
            }
        });
    }
}

impl BaseClient {
    pub async fn token_login<B: AsRef<[u8]>>(&mut self, token: B) -> Result<LoginResult, Error> {
        let token = token.as_ref();
        if token.len() != 144 || token.len() != 152 {
            return Ok(LoginResult::BadToken);
        }

        let mut data = self.data().await;
        data.sig.session = rand::random::<[u8; 4]>();
        data.sig.randkey = rand::random::<[u8; 16]>();
        data.ecdh = ECDH::new();
        data.sig.d2key = token[..16].try_into()?;
        data.sig.d2 = token[16..token.len() - 72].to_vec();
        data.sig.tgt = token[token.len() - 72..].to_vec();
        data.sig.tgtgt = md5::compute(data.sig.d2key).0;

        let mut body = Vec::with_capacity(400);
        body.write_u16(11)?;
        body.write_u16(16)?;
        body.write_bytes(tlv::pack(&data, 0x100)?)?;
        body.write_bytes(tlv::pack(&data, 0x10a)?)?;
        body.write_bytes(tlv::pack(&data, 0x116)?)?;
        body.write_bytes(tlv::pack(&data, 0x144)?)?;
        body.write_bytes(tlv::pack(&data, 0x143)?)?;
        body.write_bytes(tlv::pack(&data, 0x142)?)?;
        body.write_bytes(tlv::pack(&data, 0x154)?)?;
        body.write_bytes(tlv::pack(&data, 0x18)?)?;
        body.write_bytes(tlv::pack(&data, 0x141)?)?;
        body.write_bytes(tlv::pack(&data, 0x8)?)?;
        body.write_bytes(tlv::pack(&data, 0x147)?)?;
        body.write_bytes(tlv::pack(&data, 0x177)?)?;
        body.write_bytes(tlv::pack(&data, 0x187)?)?;
        body.write_bytes(tlv::pack(&data, 0x188)?)?;
        body.write_bytes(tlv::pack(&data, 0x202)?)?;
        body.write_bytes(tlv::pack(&data, 0x511)?)?;
        let request = data.build_login_request(LoginCommand::WtLoginExchangeEmp, body, None)?;
        drop(data);

        self.login(request).await
    }

    pub async fn password_login(&mut self, md5_password: [u8; 16]) -> Result<LoginResult, Error> {
        let mut data = self.data().await;
        data.sig.session = rand::random::<[u8; 4]>();
        data.sig.randkey = rand::random::<[u8; 16]>();
        data.sig.tgtgt = rand::random::<[u8; 16]>();
        data.ecdh = ECDH::new();

        let mut body = Vec::with_capacity(1000);
        body.write_u16(9)?;
        body.write_u16(23)?;
        body.write_bytes(tlv::pack(&data, 0x18)?)?;
        body.write_bytes(tlv::pack(&data, 0x1)?)?;
        body.write_bytes(tlv::pack_with_args(
            &data,
            0x106,
            None,
            Some(md5_password),
            None,
            None,
        )?)?;
        body.write_bytes(tlv::pack(&data, 0x116)?)?;
        body.write_bytes(tlv::pack(&data, 0x100)?)?;
        body.write_bytes(tlv::pack(&data, 0x107)?)?;
        body.write_bytes(tlv::pack(&data, 0x142)?)?;
        body.write_bytes(tlv::pack(&data, 0x144)?)?;
        body.write_bytes(tlv::pack(&data, 0x145)?)?;
        body.write_bytes(tlv::pack(&data, 0x147)?)?;
        body.write_bytes(tlv::pack(&data, 0x154)?)?;
        body.write_bytes(tlv::pack(&data, 0x141)?)?;
        body.write_bytes(tlv::pack(&data, 0x8)?)?;
        body.write_bytes(tlv::pack(&data, 0x511)?)?;
        body.write_bytes(tlv::pack(&data, 0x187)?)?;
        body.write_bytes(tlv::pack(&data, 0x188)?)?;
        body.write_bytes(tlv::pack(&data, 0x194)?)?;
        body.write_bytes(tlv::pack(&data, 0x191)?)?;
        body.write_bytes(tlv::pack(&data, 0x202)?)?;
        body.write_bytes(tlv::pack(&data, 0x177)?)?;
        body.write_bytes(tlv::pack(&data, 0x516)?)?;
        body.write_bytes(tlv::pack(&data, 0x521)?)?;
        body.write_bytes(tlv::pack(&data, 0x525)?)?;
        let request = data.build_login_request(LoginCommand::WtLoginLogin, body, None)?;
        drop(data);

        self.login(request).await
    }

    pub async fn qrcode_login(&mut self) -> Result<LoginResult, Error> {
        if let Some((retcode, uin, t106, t16a, t318, tgtgt)) = self.verify_qrcode().await? {
            let mut data = self.data().await;
            data.sig.qrsig = None;

            if retcode == 0 {
                if uin != data.uin {
                    return Ok(LoginResult::QrcodeUinNotMatched(uin, data.uin));
                }

                data.sig.tgtgt = tgtgt;

                let mut body = Vec::with_capacity(4096);
                body.write_u16(9)?;
                body.write_u16(24)?;
                body.write_bytes(tlv::pack(&data, 0x18)?)?;
                body.write_bytes(tlv::pack(&data, 0x1)?)?;
                body.write_u16(0x106)?;
                body.write_tlv(t106)?;
                body.write_bytes(tlv::pack(&data, 0x116)?)?;
                body.write_bytes(tlv::pack(&data, 0x100)?)?;
                body.write_bytes(tlv::pack(&data, 0x107)?)?;
                body.write_bytes(tlv::pack(&data, 0x142)?)?;
                body.write_bytes(tlv::pack(&data, 0x144)?)?;
                body.write_bytes(tlv::pack(&data, 0x145)?)?;
                body.write_bytes(tlv::pack(&data, 0x147)?)?;
                body.write_u16(0x16a)?;
                body.write_tlv(t16a)?;
                body.write_bytes(tlv::pack(&data, 0x154)?)?;
                body.write_bytes(tlv::pack(&data, 0x141)?)?;
                body.write_bytes(tlv::pack(&data, 0x8)?)?;
                body.write_bytes(tlv::pack(&data, 0x511)?)?;
                body.write_bytes(tlv::pack(&data, 0x187)?)?;
                body.write_bytes(tlv::pack(&data, 0x188)?)?;
                body.write_bytes(tlv::pack(&data, 0x194)?)?;
                body.write_bytes(tlv::pack(&data, 0x191)?)?;
                body.write_bytes(tlv::pack(&data, 0x202)?)?;
                body.write_bytes(tlv::pack(&data, 0x177)?)?;
                body.write_bytes(tlv::pack(&data, 0x516)?)?;
                body.write_bytes(tlv::pack(&data, 0x521)?)?;
                body.write_u16(0x318)?;
                body.write_tlv(t318)?;
                let request = data.build_login_request(LoginCommand::WtLoginLogin, body, None)?;
                drop(data);

                self.login(request).await
            } else {
                match retcode {
                    0x11 => Ok(LoginResult::QrcodeTimeout),
                    0x30 => Ok(LoginResult::QrcodeNotScanned),
                    0x35 => Ok(LoginResult::QrcodeNotConfirmed),
                    0x36 => Ok(LoginResult::QrcodeCancelled),
                    _ => Ok(LoginResult::QrcodeServerError),
                }
            }
        } else {
            Ok(LoginResult::QrcodeNotScanned)
        }
    }

    async fn login(&mut self, request: LoginRequest) -> Result<LoginResult, Error> {
        let response_body = self
            .networker
            .lock()
            .await
            .send_request(request, None)
            .await?
            .await?;

        self.decode_login_response(response_body).await
    }

    #[async_recursion]
    async fn decode_login_response(&mut self, payload: Vec<u8>) -> Result<LoginResult, Error> {
        let mut data = self.data().await;
        let decrypted = tea::decrypt(&mut &payload[16..payload.len() - 1], &data.ecdh.share_key)?;

        let mut cursor = Cursor::new(decrypted);
        cursor.seek(SeekFrom::Current(2))?;
        let typee = cursor.read_u8()?;
        cursor.seek(SeekFrom::Current(2))?;
        let mut t = cursor.read_tlv()?;

        if typee == 204 {
            data.sig.t104 = t.remove(&0x104).ok_or(TlvError::TagNotExisted(0x104))?;

            let mut body = Vec::with_capacity(500);
            body.write_u16(20)?;
            body.write_u16(4)?;
            body.write_bytes(tlv::pack(&data, 0x8)?)?;
            body.write_bytes(tlv::pack(&data, 0x104)?)?;
            body.write_bytes(tlv::pack(&data, 0x116)?)?;
            body.write_bytes(tlv::pack(&data, 0x401)?)?;

            let request = data.build_login_request(LoginCommand::WtLoginLogin, body, None)?;
            drop(data);

            self.login(request).await
        } else if typee == 0 {
            data.sig.t104.clear();
            data.sig.t174.clear();

            let t119 = t.remove(&0x119).ok_or(TlvError::TagNotExisted(0x119))?;
            let user = data.decode_t119(t119)?;
            drop(data);

            let registered = self.registry.lock().await.register().await?;
            if registered {
                Ok(LoginResult::Succeeded(user))
            } else {
                Ok(LoginResult::BadToken)
            }
        } else if typee == 15 || typee == 16 {
            Ok(LoginResult::BadToken)
        } else if typee == 2 {
            data.sig.t104 = t.remove(&0x104).ok_or(TlvError::TagNotExisted(0x104))?;

            if let Some(t192) = t.remove(&0x192) {
                Ok(LoginResult::SliderAuthorizationRequired(String::from_utf8(
                    t192,
                )?))
            } else {
                Ok(LoginResult::UnknownLoginType(typee))
            }
        } else if typee == 160 {
            let t204 = t.remove(&0x204);
            let t174 = t.remove(&0x174);
            if t204.is_none() && t174.is_none() {
                Ok(LoginResult::SecurityCodeSent)
            } else {
                let t178 = t.remove(&0x178);
                let phone = if let (Some(t174), Some(t178)) = (t174, t178) {
                    data.sig.t104 = t.remove(&0x104).ok_or(TlvError::TagNotExisted(0x104))?;
                    data.sig.t174 = t174;

                    Some(String::from_utf8(t178)?)
                } else {
                    None
                };

                let t204 = t
                    .remove(&0x204)
                    .and_then(|buf| String::from_utf8(buf).or::<String>(Ok(String::new())).ok());

                Ok(LoginResult::SecurityAuthorizationRequired(t204, phone))
            }
        } else if t.contains_key(&0x149) {
            let t149 = t.remove(&0x149).ok_or(TlvError::TagNotExisted(0x149))?;
            let reader = &mut &t149[2..];

            let len = reader.read_u16()?;
            let title = String::from_utf8(reader.read_bytes(len as usize)?)?;
            let len = reader.read_u16()?;
            let content = String::from_utf8(reader.read_bytes(len as usize)?)?;

            Ok(LoginResult::Failure(title, content))
        } else if t.contains_key(&0x146) {
            let t146 = t.remove(&0x146).ok_or(TlvError::TagNotExisted(0x146))?;
            let reader = &mut &t146[4..];

            let len = reader.read_u16()?;
            let title = String::from_utf8(reader.read_bytes(len as usize)?)?;
            let len = reader.read_u16()?;
            let content = String::from_utf8(reader.read_bytes(len as usize)?)?;

            Ok(LoginResult::Failure(title, content))
        } else {
            Ok(LoginResult::Failure(
                "登录失败".to_string(),
                "未知错误".to_string(),
            ))
        }
    }

    pub async fn fetch_qrcode(&self) -> Result<Vec<u8>, Error> {
        let mut data = self.data().await;
        let mut body = Vec::with_capacity(1024);
        body.write_u16(0)?;
        body.write_u32(16)?;
        body.write_u64(0)?;
        body.write_u8(8)?;
        body.write_tlv(&BUF_0)?;
        body.write_u16(6)?;
        body.write_bytes(tlv::pack(&data, 0x16)?)?;
        body.write_bytes(tlv::pack(&data, 0x1B)?)?;
        body.write_bytes(tlv::pack(&data, 0x1D)?)?;
        body.write_bytes(tlv::pack(&data, 0x1F)?)?;
        body.write_bytes(tlv::pack(&data, 0x33)?)?;
        body.write_bytes(tlv::pack(&data, 0x35)?)?;

        let request = data.build_qrcode_request(0x31, 0x11100, body)?;
        drop(data);

        let response = self
            .networker
            .lock()
            .await
            .send_request(request, None)
            .await?;
        let response_body = response.await?;

        let decrypted = tea::decrypt(
            &response_body[16..response_body.len() - 1],
            &self.data().await.ecdh.share_key,
        )?;

        let mut cursor = Cursor::new(decrypted);
        cursor.seek(SeekFrom::Current(54))?;
        let retcode = cursor.read_u8()?;
        let qrsig_len = cursor.read_u16()?;
        let qrsig = cursor.read_bytes(qrsig_len as usize)?;
        cursor.seek(SeekFrom::Current(2))?;
        let mut t = cursor.read_tlv()?;

        if retcode == 0 && t.contains_key(&0x17) {
            let t17 = t.remove(&0x17).ok_or(TlvError::TagNotExisted(0x17))?;
            self.data().await.sig.qrsig = Some(qrsig);
            Ok(t17)
        } else {
            Err(ClientError::FetchQrcodeFailure)?
        }
    }

    pub async fn verify_qrcode(
        &self,
    ) -> Result<Option<(u8, u32, Vec<u8>, Vec<u8>, Vec<u8>, [u8; 16])>, Error> {
        let mut data = self.data().await;
        if let Some(qrsig) = &data.sig.qrsig {
            let mut body = Vec::with_capacity(26 + qrsig.len());
            body.write_u16(5)?;
            body.write_u8(1)?;
            body.write_u32(8)?;
            body.write_u32(16)?;
            body.write_tlv(qrsig)?;
            body.write_u64(0)?;
            body.write_u8(8)?;
            body.write_tlv(BUF_0)?;
            body.write_u16(0)?;

            let request = data.build_qrcode_request(0x12, 0x6200, body)?;
            drop(data);

            let response = self
                .networker
                .lock()
                .await
                .send_request(request, None)
                .await?;
            let response_body = response.await?;

            let decrypted = tea::decrypt(
                &response_body[16..response_body.len() - 1],
                &self.data().await.ecdh.share_key,
            )?;
            let mut cursor = Cursor::new(decrypted);
            cursor.seek(SeekFrom::Current(48))?;
            let mut len = cursor.read_u16()?;
            if len > 0 {
                len -= 1;
                if cursor.read_u8()? == 2 {
                    cursor.seek(SeekFrom::Current(8))?;
                    len -= 8;
                }

                if len > 0 {
                    cursor.seek(SeekFrom::Current(len as i64))?;
                }
            }

            cursor.seek(SeekFrom::Current(4))?;
            let retcode = cursor.read_u8()?;
            if retcode == 0 {
                cursor.seek(SeekFrom::Current(4))?;
                let uin = cursor.read_u32()?;
                cursor.seek(SeekFrom::Current(6))?;

                let mut t = cursor.read_tlv()?;
                if let (Some(t106), Some(t16a), Some(t318), Some(tgtgt)) = (
                    t.remove(&0x18),
                    t.remove(&0x19),
                    t.remove(&0x65),
                    t.remove(&0x1e),
                ) {
                    return Ok(Some((
                        retcode,
                        uin,
                        t106,
                        t16a,
                        t318,
                        tgtgt.try_into().unwrap(),
                    )));
                }
            }

            Ok(None)
        } else {
            Ok(None)
        }
    }

    pub async fn send_security_code(&mut self) -> Result<LoginResult, Error> {
        let mut data = self.data().await;

        let t8 = tlv::pack(&data, 0x8)?;
        let t104 = tlv::pack(&data, 0x104)?;
        let t116 = tlv::pack(&data, 0x116)?;
        let t174 = tlv::pack(&data, 0x174)?;
        let t17a = tlv::pack(&data, 0x17a)?;
        let t197 = tlv::pack(&data, 0x197)?;

        let mut body = Vec::with_capacity(
            2 + 2 + t8.len() + t104.len() + t116.len() + t174.len() + t17a.len() + t197.len(),
        );
        body.write_u16(8)?;
        body.write_u16(6)?;
        body.write_bytes(t8)?;
        body.write_bytes(t104)?;
        body.write_bytes(t116)?;
        body.write_bytes(t174)?;
        body.write_bytes(t17a)?;
        body.write_bytes(t197)?;
        let request = data.build_login_request(LoginCommand::WtLoginLogin, body, None)?;
        drop(data);

        self.login(request).await
    }

    pub async fn submit_security_code<S: AsRef<str>>(
        &mut self,
        code: S,
    ) -> Result<LoginResult, Error> {
        let code = code.as_ref().trim();
        if code.len() != 6 {
            Ok(LoginResult::SecurityCodeIncorrect)
        } else {
            let mut data = self.data().await;

            let t8 = tlv::pack(&data, 0x8)?;
            let t104 = tlv::pack(&data, 0x104)?;
            let t116 = tlv::pack(&data, 0x116)?;
            let t174 = tlv::pack(&data, 0x174)?;
            let t17c = tlv::pack(&data, 0x17c)?;
            let t402 = tlv::pack(&data, 0x401)?;
            let t198 = tlv::pack(&data, 0x198)?;

            let mut body = Vec::with_capacity(
                2 + 2
                    + t8.len()
                    + t104.len()
                    + t116.len()
                    + t174.len()
                    + t17c.len()
                    + t402.len()
                    + t198.len(),
            );
            body.write_u16(8)?;
            body.write_u16(6)?;
            body.write_bytes(t8)?;
            body.write_bytes(t104)?;
            body.write_bytes(t116)?;
            body.write_bytes(t174)?;
            body.write_bytes(t17c)?;
            body.write_bytes(t402)?;
            body.write_bytes(t198)?;
            let request = data.build_login_request(LoginCommand::WtLoginLogin, body, None)?;
            drop(data);

            self.login(request).await
        }
    }

    pub async fn submit_slider<S: AsRef<str>>(&mut self, ticket: S) -> Result<LoginResult, Error> {
        let mut data = self.data().await;

        let t193 = tlv::pack_with_args(&data, 0x193, None, None, None, Some(ticket.as_ref()))?;
        let t8 = tlv::pack(&data, 0x8)?;
        let t104 = tlv::pack(&data, 0x104)?;
        let t116 = tlv::pack(&data, 0x116)?;

        let mut body = Vec::with_capacity(2 + 2 + t193.len() + t8.len() + t104.len() + t116.len());
        body.write_u16(2)?;
        body.write_u16(4)?;
        body.write_bytes(t193)?;
        body.write_bytes(t8)?;
        body.write_bytes(t104)?;
        body.write_bytes(t116)?;
        let request = data.build_login_request(LoginCommand::WtLoginLogin, body, None)?;
        drop(data);

        self.login(request).await
    }

    pub async fn logout(&mut self) -> Result<(), Error> {
        self.registry.lock().await.unregister().await
    }
}

impl BaseClient {}

impl AsRef<BaseClient> for BaseClient {
    fn as_ref(&self) -> &BaseClient {
        self
    }
}

impl AsMut<BaseClient> for BaseClient {
    fn as_mut(&mut self) -> &mut BaseClient {
        self
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub token: Vec<u8>,
    pub nickname: String,
    pub gender: u8,
    pub age: u8,
}

impl Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("nickname: {}\n", self.nickname))?;
        f.write_fmt(format_args!("gender: {}\n", self.gender))?;
        f.write_fmt(format_args!("age: {}\n", self.age))?;
        f.write_fmt(format_args!("token: {}\n", base64::encode(&self.token)))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataCenter {
    pub pskey: HashMap<String, Vec<u8>>,
    pub uin: u32,
    pub apk: APK,
    pub device: FullDevice,
    pub sig: SIG,
    pub ecdh: ECDH,
}

impl DataCenter {
    fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        Self {
            pskey: HashMap::new(),
            uin,
            apk: platform.metadata(),
            device: FullDevice::from(d.unwrap_or(ShortDevice::generate(uin))),
            sig: SIG::new(uin),
            ecdh: ECDH::new(),
        }
    }
}

impl DataCenter {
    fn increase_seq(&mut self) -> u32 {
        let mut next = self.sig.seq + 1;
        if next >= 0x8000 {
            next = 0
        }

        self.sig.seq = next;
        next
    }

    fn build_login_request<B>(
        &mut self,
        command: LoginCommand,
        body: B,
        typee: Option<CommandType>,
    ) -> Result<LoginRequest, Error>
    where
        B: AsRef<[u8]>,
    {
        let typee = typee.unwrap_or(CommandType::Type2);

        let seq = self.increase_seq();

        let (uin, cmd_id, subid) = match command {
            LoginCommand::WtLoginTransEmp => (0, 0x812, Platform::watch().subid),
            _ => (self.uin, 0x810, self.apk.subid),
        };

        let mut buf_1;
        let body = match typee {
            CommandType::Type2 => {
                let encrypted = tea::encrypt(&body, &self.ecdh.share_key)?;
                let mut buf_0 =
                    Vec::with_capacity(24 + self.ecdh.public_key.len() + encrypted.len());
                buf_0.write_u8(0x02)?;
                buf_0.write_u8(0x01)?;
                buf_0.write_bytes(&self.sig.randkey)?;
                buf_0.write_u16(0x131)?;
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
                buf_1.write_bytes(&buf_0)?;
                buf_1.write_u8(0x03)?;

                buf_1.as_slice()
            }
            _ => body.as_ref(),
        };

        let mut buf =
            Vec::with_capacity(54 + command.as_ref().len() + self.device.imei.as_str().len());
        buf.write_u32(seq)?;
        buf.write_u32(subid)?;
        buf.write_u32(subid)?;
        buf.write_bytes([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        ])?;
        buf.write_bytes_with_length(&self.sig.tgt)?;
        buf.write_bytes_with_length(command.as_ref())?;
        buf.write_bytes_with_length(self.sig.session)?;
        buf.write_bytes_with_length(self.device.imei.as_str())?;
        buf.write_u32(4)?;
        buf.write_u16(2)?;
        buf.write_u32(4)?;

        let mut sso = Vec::with_capacity(buf.len() + body.len() + 8);
        sso.write_bytes_with_length(buf)?;
        sso.write_bytes_with_length(body)?;

        let encrypted_sso = match typee {
            CommandType::Type1 => tea::encrypt(sso, &self.sig.d2key)?,
            CommandType::Type2 => tea::encrypt(sso, &BUF_16)?,
            _ => sso,
        };

        let uin = uin.to_string();
        let mut payload =
            Vec::with_capacity(14 + &self.sig.d2.len() + encrypted_sso.len() + uin.len());
        payload.write_u32(0x0A)?;
        payload.write_u8(typee as u8)?;
        payload.write_bytes_with_length(&self.sig.d2)?;
        payload.write_u8(0)?;
        payload.write_bytes_with_length(uin)?;
        payload.write_bytes(encrypted_sso)?;
        let mut result = Vec::with_capacity(payload.len() + 4);
        result.write_bytes_with_length(payload)?;

        Ok(LoginRequest::new(seq, command, result))
    }

    fn build_qrcode_request<B: AsRef<[u8]>>(
        &mut self,
        cmd_id: u16,
        head: u32,
        body: B,
    ) -> Result<LoginRequest, Error> {
        let body = body.as_ref();
        let mut buf = Vec::with_capacity(62 + body.len());
        buf.write_u32(head)?;
        buf.write_u32(0x1000)?;
        buf.write_u16(0)?;
        buf.write_u32(0x72000000)?;
        buf.write_u32(current_unix_timestamp_as_secs() as u32)?;
        buf.write_u8(2)?;
        buf.write_u16((44 + body.len()) as u16)?;
        buf.write_u16(cmd_id)?;
        buf.write_bytes([0; 21])?;
        buf.write_u8(3)?;
        buf.write_u16(0)?;
        buf.write_u16(50)?;
        buf.write_u32(self.sig.seq + 1)?;
        buf.write_u64(0)?;
        buf.write_bytes(body)?;
        buf.write_u8(3)?;
        let request = self.build_login_request(LoginCommand::WtLoginTransEmp, buf, None)?;
        Ok(request)
    }

    pub fn build_uni_request<S, B>(
        &mut self,
        command: S,
        body: B,
        seq: Option<u32>,
    ) -> Result<UniRequest, Error>
    where
        B: AsRef<[u8]>,
        S: AsRef<str>,
    {
        let command = command.as_ref();
        let seq = seq.unwrap_or(self.increase_seq());
        let body = body.as_ref();

        let len = command.len() + 20;
        let mut sso = Vec::with_capacity(len + body.len() + 4);
        sso.write_u32(len as u32)?;
        sso.write_u32((command.len() + 4) as u32)?;
        sso.write_bytes(command)?;
        sso.write_u32(8)?;
        sso.write_bytes(self.sig.session)?;
        sso.write_u32(4)?;
        sso.write_u32((body.len() + 4) as u32)?;
        sso.write_bytes(body)?;

        let encrypt = tea::encrypt(sso, &self.sig.d2key)?;
        let uin = self.uin.to_string();
        let len = encrypt.len() + uin.len() + 18;
        let mut payload = Vec::with_capacity(len);
        payload.write_u32(len as u32)?;
        payload.write_u32(0x0B)?;
        payload.write_u8(1)?;
        payload.write_u32(seq)?;
        payload.write_u8(0)?;
        payload.write_u32((uin.len() + 4) as u32)?;
        payload.write_bytes(uin)?;
        payload.write_bytes(encrypt)?;

        Ok(UniRequest::new(seq, command.to_string(), payload))
    }

    fn build_register_request(&mut self, logout: bool) -> Result<LoginRequest, Error> {
        let pb_buf = protobuf::encode(&ProtobufObject::from([(
            1,
            ProtobufElement::from([
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(46)),
                    (2, ProtobufElement::from(current_unix_timestamp_as_secs())),
                ])),
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(283)),
                    (2, ProtobufElement::from(0)),
                ])),
            ]),
        )]))?;

        let d = &self.device;
        let svc_req_register = jce::encode_nested(JceObject::try_from([
            Some(JceElement::from(self.uin as i64)),
            Some(JceElement::from(logout.then_some(0).unwrap_or(7))),
            Some(JceElement::from(0)),
            Some(JceElement::from("")),
            Some(JceElement::from(logout.then_some(21).unwrap_or(11))),
            Some(JceElement::from(0)),
            Some(JceElement::from(0)),
            Some(JceElement::from(0)),
            Some(JceElement::from(0)),
            Some(JceElement::from(0)),
            Some(JceElement::from(logout.then_some(44).unwrap_or(0))),
            Some(JceElement::from(d.version.sdk as i64)),
            Some(JceElement::from(1)),
            Some(JceElement::from("")),
            Some(JceElement::from(0)),
            None,
            Some(JceElement::from(d.guid)),
            Some(JceElement::from(2052)),
            Some(JceElement::from(0)),
            Some(JceElement::from(d.model)),
            Some(JceElement::from(d.model)),
            Some(JceElement::from(d.version.release)),
            Some(JceElement::from(1)),
            Some(JceElement::from(0)),
            Some(JceElement::from(0)),
            None,
            Some(JceElement::from(0)),
            Some(JceElement::from(0)),
            Some(JceElement::from("")),
            Some(JceElement::from(0)),
            Some(JceElement::from(d.brand)),
            Some(JceElement::from(d.brand)),
            Some(JceElement::from("")),
            Some(JceElement::from(pb_buf)),
            Some(JceElement::from(0)),
            None,
            Some(JceElement::from(0)),
            None,
            Some(JceElement::from(1000)),
            Some(JceElement::from(98)),
        ])?)?;
        let body = jce::encode_wrapper(
            [("SvcReqRegister", svc_req_register)],
            "PushService",
            "SvcReqRegister",
            None,
        )?;
        let pkt = self.build_login_request(
            LoginCommand::StatSvcRegister,
            body,
            Some(CommandType::Type1),
        )?;

        Ok(pkt)
    }

    fn decode_t119<B>(&mut self, t119: B) -> Result<User, Error>
    where
        B: AsRef<[u8]>,
    {
        let decrypted = tea::decrypt(t119, &self.sig.tgtgt)?;
        let mut t = (&mut &decrypted[2..]).read_tlv()?;

        self.sig.tgt = t.remove(&0x10a).ok_or(TlvError::TagNotExisted(0x10a))?;
        self.sig.skey = t.remove(&0x120).ok_or(TlvError::TagNotExisted(0x120))?;
        self.sig.d2 = t.remove(&0x143).ok_or(TlvError::TagNotExisted(0x143))?;
        self.sig.d2key =
            t.remove(&0x305).ok_or(TlvError::TagNotExisted(0x305))?[..16].try_into()?;
        self.sig.tgtgt = md5::compute(&self.sig.d2key).0;
        self.sig.emp_time = current_unix_timestamp_as_secs();

        if let Some(value) = t.get(&0x512) {
            let reader = &mut value.as_slice();
            let mut len = reader.read_u16()?;

            while len > 0 {
                let domain_len = reader.read_u16()? as usize;
                let domain = String::from_utf8(reader.read_bytes(domain_len)?)?;

                let pskey_len = reader.read_u16()? as usize;
                let pskey = reader.read_bytes(pskey_len)?;

                self.pskey.insert(domain, pskey);

                len -= 1;
            }
        }

        let mut token: Vec<u8> =
            Vec::with_capacity(self.sig.d2key.len() + self.sig.d2.len() + self.sig.tgt.len());
        token.extend(&self.sig.d2key);
        token.extend(&self.sig.d2);
        token.extend(&self.sig.tgt);
        let ddd = t.remove(&0x11a).ok_or(TlvError::TagNotExisted(0x11a))?;
        let age = ddd[2];
        let gender = ddd[3];
        let nickname = String::from_utf8(ddd[5..].to_vec())?;

        Ok(User {
            token,
            nickname,
            gender,
            age,
        })
    }
}

#[derive(Debug)]
struct Networker {
    data: Arc<Mutex<DataCenter>>,
    network: Network,
    registered: AtomicBool,
}

impl Networker {
    fn new(statistics: Arc<Mutex<Statistics>>, data: Arc<Mutex<DataCenter>>) -> Self {
        Self {
            network: Network::new(Arc::clone(&data), statistics),
            data,
            registered: AtomicBool::new(false),
        }
    }

    fn on_push(&self) -> Receiver<SSO> {
        self.network.on_sso()
    }

    fn on_state(&self) -> Receiver<(NetworkState, Option<SocketAddr>)> {
        self.network.on_state()
    }

    fn registered(&self) -> bool {
        self.registered.load(Ordering::Relaxed)
    }

    fn set_registered(&mut self, registered: bool) {
        self.registered.store(registered, Ordering::Relaxed);
    }

    async fn connect(&mut self) -> Result<(), Error> {
        self.network.connect().await?;
        self.sync_time_diff().await
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.network.disconnect().await
    }
}

impl Networker {
    async fn sync_time_diff(&mut self) -> Result<(), Error> {
        let request_packet = self.data.lock().await.build_login_request(
            LoginCommand::ClientCorrectTime,
            BUF_4,
            Some(CommandType::Type0),
        )?;
        let request = self.send_request(request_packet, None).await?;
        let response_body = request.await?;

        // 此处忽略错误
        if let Ok(server_time) = (&mut response_body.as_slice()).read_i32() {
            self.data.lock().await.sig.time_diff =
                server_time as i64 - current_unix_timestamp_as_secs() as i64;
        }

        Ok(())
    }

    /// 等待返回结果
    async fn send_request<B>(
        &mut self,
        request: B,
        timeout: Option<Duration>,
    ) -> Result<Response, Error>
    where
        B: Request,
    {
        self.network
            .send_request(request, timeout.unwrap_or(Duration::from_secs(5)))
            .await
    }

    /// 不等待返回结果
    async fn write_request<B>(&mut self, request: B) -> Result<(), Error>
    where
        B: Request,
    {
        self.network.write_request(request).await
    }

    async fn send_register(&mut self, request: LoginRequest, refresh: bool) -> Result<bool, Error> {
        self.set_registered(false);

        let request = self
            .send_request(request, Some(Duration::from_secs(10)))
            .await?;
        let response = request.await?;

        let rsp = jce::decode_wrapper(&mut response.as_slice())?;
        if let JceElement::StructBegin(mut value) = rsp {
            let v: i8 = value.try_remove(&9)?.try_into()?;

            if v == 0 && !refresh {
                Ok(false)
            } else {
                self.set_registered(true);
                Ok(true)
            }
        } else {
            Err(JceError::DecodeError)?
        }
    }

    async fn send_unregister(&mut self, request: LoginRequest) -> Result<(), Error> {
        self.set_registered(false);
        self.write_request(request).await?;
        Ok(())
    }

    /// 发送一个业务包但不等待返回
    pub async fn write_registered_request<B>(&mut self, request: B) -> Result<(), Error>
    where
        B: Request,
    {
        if !self.registered() {
            return Err(ClientError::NotRegistered)?;
        }

        self.write_request(request).await?;
        Ok(())
    }

    /// 发送一个业务包并等待返回结果
    pub async fn send_registered_request<B>(
        &mut self,
        request: B,
        timeout: Option<Duration>,
    ) -> Result<Response, Error>
    where
        B: Request,
    {
        if !self.registered() {
            return Err(ClientError::NotRegistered)?;
        }

        let response = self.send_request(request, timeout).await?;
        Ok(response)
    }
}

#[derive(Debug)]
struct Registry {
    data: Arc<Mutex<DataCenter>>,
    networker: Arc<Mutex<Networker>>,
    heartbeat_handler: Option<JoinHandle<()>>,

    token_tx: Sender<Vec<u8>>,
}

impl Registry {
    fn new(data: Arc<Mutex<DataCenter>>, networker: Arc<Mutex<Networker>>) -> Self {
        Self {
            data,
            networker,
            heartbeat_handler: None,

            token_tx: sync::broadcast::channel(1).0,
        }
    }

    fn on_token_update(&self) -> Receiver<Vec<u8>> {
        self.token_tx.subscribe()
    }

    async fn register(&mut self) -> Result<bool, Error> {
        if self.networker.lock().await.registered() {
            return Ok(true);
        }

        if let Some(handler) = self.heartbeat_handler.take() {
            handler.abort();
            let _ = handler.await;
        }

        let request = self.data.lock().await.build_register_request(false)?;
        let registered = self
            .networker
            .lock()
            .await
            .send_register(request, false)
            .await?;
        if registered {
            let heartbeater = Heartbeater::new(
                Arc::clone(&self.data),
                Arc::clone(&self.networker),
                self.token_tx.clone(),
            );
            self.heartbeat_handler = Some(heartbeater.start_heartbeat());
        }

        Ok(registered)
    }

    async fn unregister(&mut self) -> Result<(), Error> {
        if !self.networker.lock().await.registered() {
            return Ok(());
        }

        if let Some(handler) = self.heartbeat_handler.take() {
            handler.abort();
            let _ = handler.await;
        };

        let request = self.data.lock().await.build_register_request(true)?;
        self.networker.lock().await.send_unregister(request).await?;
        Ok(())
    }
}

#[derive(Debug)]
struct Heartbeater {
    retried: u8,
    data: Arc<Mutex<DataCenter>>,
    networker: Arc<Mutex<Networker>>,
    token_tx: Sender<Vec<u8>>,
}

impl Heartbeater {
    fn new(
        data: Arc<Mutex<DataCenter>>,
        networker: Arc<Mutex<Networker>>,
        token_tx: Sender<Vec<u8>>,
    ) -> Self {
        Self {
            retried: 0,
            data,
            networker,
            token_tx,
        }
    }

    fn start_heartbeat(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while self.networker.lock().await.registered() {
                match self.beat().await {
                    Ok(_) => tokio::time::sleep(Duration::from_secs(1)).await,
                    Err(_) => break,
                }
            }

            self.networker.lock().await.set_registered(false);
            let _ = self.networker.lock().await.disconnect().await;
            error!("heartbeat failure, disconnected.");
        })
    }

    async fn refresh_token(&mut self) -> Result<(), Error> {
        let mut data = self.data.lock().await;

        if current_unix_timestamp_as_secs() - data.sig.emp_time < 14000 {
            return Ok(());
        }

        let mut body = Vec::with_capacity(2000);
        body.write_u16(11)?;
        body.write_u16(16)?;
        body.write_bytes(tlv::pack(&data, 0x100)?)?;
        body.write_bytes(tlv::pack(&data, 0x10a)?)?;
        body.write_bytes(tlv::pack(&data, 0x116)?)?;
        body.write_bytes(tlv::pack(&data, 0x144)?)?;
        body.write_bytes(tlv::pack(&data, 0x143)?)?;
        body.write_bytes(tlv::pack(&data, 0x142)?)?;
        body.write_bytes(tlv::pack(&data, 0x154)?)?;
        body.write_bytes(tlv::pack(&data, 0x18)?)?;
        body.write_bytes(tlv::pack(&data, 0x141)?)?;
        body.write_bytes(tlv::pack(&data, 0x8)?)?;
        body.write_bytes(tlv::pack(&data, 0x147)?)?;
        body.write_bytes(tlv::pack(&data, 0x177)?)?;
        body.write_bytes(tlv::pack(&data, 0x187)?)?;
        body.write_bytes(tlv::pack(&data, 0x188)?)?;
        body.write_bytes(tlv::pack(&data, 0x202)?)?;
        body.write_bytes(tlv::pack(&data, 0x511)?)?;
        let packet = data.build_login_request(LoginCommand::WtLoginExchangeEmp, body, None)?;
        drop(data);

        let request = self
            .networker
            .lock()
            .await
            .send_request(packet, None)
            .await?;
        let response_body = request.await?;

        let decrypted = tea::decrypt(
            &response_body[16..response_body.len() - 1],
            &self.data.lock().await.ecdh.share_key,
        )?;
        let typee = decrypted[2];
        let mut t = (&mut &decrypted[5..]).read_tlv()?;
        if typee == 0 {
            let t119 = t.remove(&0x119).ok_or(TlvError::TagNotExisted(0x119))?;
            let user = self.data.lock().await.decode_t119(t119)?;

            let request = self.data.lock().await.build_register_request(false)?;
            let registered = self
                .networker
                .lock()
                .await
                .send_register(request, true)
                .await?;

            if registered {
                let _ = self.token_tx.send(user.token);
            }
        }

        Ok(())
    }

    #[async_recursion]
    async fn beat(&mut self) -> Result<(), Error> {
        self.networker.lock().await.sync_time_diff().await?;

        let mut data = self.data.lock().await;
        let hb480 = data.sig.hb480.clone();
        let request_packet = data.build_uni_request("OidbSvc.0x480_9_IMCore", hb480, None)?;
        drop(data);

        let request = self
            .networker
            .lock()
            .await
            .send_registered_request(request_packet, None)
            .await?;
        match request.await {
            Ok(_) => {
                self.retried = 0;
                self.refresh_token().await?;
                Ok(())
            }
            Err(err) => {
                self.retried += 1;
                error!(
                    "heartbeat timeout, retried count: {}, reason: {}",
                    self.retried, err
                );

                if self.retried >= 2 {
                    Err(ClientError::HeartbeatFailure)?
                } else {
                    self.beat().await
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io::stdin, time::Duration};

    use log::{info, warn};

    use crate::{core::io::WriteExt, error::Error, init_logger, tmp_dir};

    use super::{BaseClient, LoginResult};

    #[tokio::test]
    async fn test_password_login() -> Result<(), Error> {
        init_logger()?;

        let mut base_client = BaseClient::default(640279992).await;

        base_client.connect().await?;

        let password = rpassword::prompt_password("Please enter password > ")?;
        let md5_password = md5::compute(&password).0;
        let mut result = base_client.password_login(md5_password).await?;
        loop {
            if let LoginResult::Succeeded(user) = &result {
                println!("{}", user);
                break;
            } else if let LoginResult::SliderAuthorizationRequired(url) = &result {
                println!("Please finish slider authorization and get the ticket");
                println!("{}", url);

                let ticket = rpassword::prompt_password("Enter ticket to continue > ")?;
                result = base_client.submit_slider(ticket.as_str()).await?;
            } else if let LoginResult::SecurityAuthorizationRequired(url, _) = &result {
                println!("Please finish security authorization using url verification or sms code");
                println!("Enter 1(or empty) to use sms code, or enter 2 to use url > ");

                let mut confirm = String::with_capacity(1);
                loop {
                    confirm.clear();
                    stdin().read_line(&mut confirm)?;

                    if confirm.is_empty() || confirm == "1" {
                        if let LoginResult::SecurityCodeSent =
                            base_client.send_security_code().await?
                        {
                            let mut code = String::with_capacity(6);
                            loop {
                                print!("Please enter code > ");
                                stdin().read_line(&mut code)?;

                                if let LoginResult::SecurityCodeIncorrect =
                                    base_client.submit_security_code(code.as_str()).await?
                                {
                                    println!("SMS code incorrect! please retry.");
                                } else {
                                    break;
                                }
                            }
                        } else {
                            panic!("SMS code sent failure, please retry later.")
                        }
                    } else if confirm == "2" {
                        println!("{:?}", url);
                        print!("Enter to continue when finished > ");
                    }
                }
            } else if let LoginResult::Failure(typee, reason) = &result {
                panic!("[{}] {}", typee, reason);
            } else {
                panic!("{:?}", result);
            }
        }

        if let Ok((typee, reason)) = base_client.on_kickoff().await.recv().await {
            warn!("Kickoff: {}, {}", typee, reason);
        } else {
            panic!("kick off failure.")
        }

        base_client.disconnect().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_qrcode_login() -> Result<(), Error> {
        init_logger()?;

        let mut base_client = BaseClient::default(640279992).await;

        base_client.connect().await?;
        let qrcode = base_client.fetch_qrcode().await?;

        let mut file = fs::OpenOptions::new()
            .append(false)
            .create(true)
            .write(true)
            .open(tmp_dir()?.join("qrcode.jpg"))
            .unwrap();
        let _ = file.write_bytes(&qrcode);
        info!("Qrcode downloaded");

        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let result = base_client.qrcode_login().await?;
            if let LoginResult::Succeeded(user) = result {
                println!("{}", user);
                break;
            } else if let LoginResult::QrcodeNotScanned = result {
                continue;
            } else if let LoginResult::QrcodeNotConfirmed = result {
                continue;
            } else {
                panic!("login failure: {:?}", result)
            }
        }

        if let Ok((typee, reason)) = base_client.on_kickoff().await.recv().await {
            warn!("Kickoff: {}, {}", typee, reason);
        } else {
            panic!("kick off failure.")
        }

        base_client.disconnect().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test() -> Result<(), Error> {
        init_logger()?;

        let base_client = BaseClient::default(1313).await;

        base_client.connect().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        base_client.disconnect().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        drop(base_client);

        Ok(())
    }
}
