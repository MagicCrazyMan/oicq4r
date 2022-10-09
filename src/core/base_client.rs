use std::{
    collections::HashMap,
    fmt::Display,
    future::Future,
    io::{Read, Write},
    net::SocketAddr,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    task::{Context, Poll},
    time::{Duration, SystemTime},
};

use async_recursion::async_recursion;
use flate2::Decompress;
use log::{debug, error, info, Level};
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
    device::{FullDevice, Platform, ShortDevice, APK},
    ecdh::ECDH,
    error::CommonError,
    helper::{current_unix_timestamp_as_secs, BUF_0, BUF_16, BUF_4},
    io::{ReadExt, WriteExt},
    jce::{self, JceElement, JceObject},
    network::{Network, NetworkState},
    protobuf::{self, ProtobufElement, ProtobufObject},
    tea::{self, decrypt},
    tlv::{self, ReadTlvExt, WriteTlvExt},
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
    pub tgt: Vec<u8>,
    pub skey: Vec<u8>,
    pub d2: Vec<u8>,
    pub d2key: [u8; 16],
    pub t104: [u8; 0],
    pub t174: [u8; 0],
    pub qrsig: [u8; 0],
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
            buf.write_i32(0x19e39).unwrap();
            buf
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
    lost_pkt_cnt: usize,
    lost_times: usize,
    msg_cnt_per_min: usize,
    recv_msg_cnt: usize,
    recv_pkt_cnt: usize,
    remote_socket_addr: Option<SocketAddr>,
    sent_msg_cnt: usize,
    sent_pkt_cnt: usize,
    start_time: SystemTime,
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

impl Statistics {
    pub fn lost_pkt_cnt(&self) -> usize {
        self.lost_pkt_cnt
    }

    pub fn lost_times(&self) -> usize {
        self.lost_times
    }

    pub fn msg_cnt_per_min(&self) -> usize {
        self.msg_cnt_per_min
    }

    pub fn recv_msg_cnt(&self) -> usize {
        self.recv_msg_cnt
    }

    pub fn recv_pkt_cnt(&self) -> usize {
        self.recv_pkt_cnt
    }

    pub fn remote_socket_addr(&self) -> Option<SocketAddr> {
        self.remote_socket_addr
    }

    pub fn sent_msg_cnt(&self) -> usize {
        self.sent_msg_cnt
    }

    pub fn sent_pkt_cnt(&self) -> usize {
        self.sent_pkt_cnt
    }

    pub fn start_time(&self) -> SystemTime {
        self.start_time
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
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

trait Request {
    fn seq(&self) -> u32;

    fn command(&self) -> &str;

    fn payload(&self) -> &[u8];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginCommand {
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

struct LoginRequest {
    seq: u32,
    command: LoginCommand,
    payload: Vec<u8>,
}

impl LoginRequest {
    fn new(seq: u32, command: LoginCommand, payload: Vec<u8>) -> Self {
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
enum UniCommand {
    OidbSvc,
}

impl AsRef<str> for UniCommand {
    fn as_ref(&self) -> &str {
        match self {
            UniCommand::OidbSvc => "OidbSvc.0x480_9_IMCore",
        }
    }
}

struct UniRequest {
    seq: u32,
    command: UniCommand,
    payload: Vec<u8>,
}

impl UniRequest {
    fn new(seq: u32, command: UniCommand, payload: Vec<u8>) -> Self {
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
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Response {
    timeout: Duration,
    start: Instant,
    response: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Future for Response {
    type Output = Result<Vec<u8>, CommonError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start.elapsed() >= self.timeout {
            Poll::Ready(Err(CommonError::new("timeout")))
        } else {
            match self.response.try_lock() {
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
struct Heartbeater {
    retried: u8,
    data: Arc<Mutex<Data>>,
    request_sender: Requester,
}

impl Heartbeater {
    fn new(data: Arc<Mutex<Data>>, request_sender: Requester) -> Self {
        Self {
            retried: 0,
            data,
            request_sender,
        }
    }

    async fn sync_time_diff(&mut self) -> Result<(), CommonError> {
        let request_packet = self.data.lock().await.build_common_packet(
            LoginCommand::ClientCorrectTime,
            BUF_4,
            Some(CommandType::Type0),
        )?;
        let request = self
            .request_sender
            .send_request(request_packet, None)
            .await?;
        let response = request.await?;

        // 此处忽略错误
        if let Ok(server_time) = (&mut response.as_slice()).read_i32() {
            self.data.lock().await.sig.time_diff =
                server_time as i64 - current_unix_timestamp_as_secs() as i64;
        }

        Ok(())
    }

    async fn refresh_token(&mut self) -> Result<(), CommonError> {
        let mut data = self.data.lock().await;

        if current_unix_timestamp_as_secs() - data.sig.emp_time < 14000 {
            return Ok(());
        }

        let mut body = Vec::with_capacity(2000);
        body.write_u16(11)?;
        body.write_u16(16)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x100)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x10a)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x116)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x144)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x143)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x142)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x154)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x18)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x141)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x8)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x147)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x177)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x187)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x188)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x202)?)?;
        body.write_bytes(tlv::pack_tlv(&data, 0x511)?)?;
        let packet = data.build_common_packet(LoginCommand::WtLoginExchangeEmp, body, None)?;
        drop(data);

        let request = self.request_sender.send_request(packet, None).await?;
        let response = request.await?;

        let decrypted = tea::decrypt(&response[16..], &self.data.lock().await.ecdh.share_key)?;
        let r#type = decrypted[2];
        let t = (&mut &decrypted[5..]).read_tlv()?;
        if r#type == 0 {
            let t119 = t
                .get(&0x119)
                .ok_or(CommonError::new("tag 0x119 not existed"))?;
            let user = self.data.lock().await.decode_t119(t119)?;

            todo!()
        }

        Ok(())
    }

    #[async_recursion]
    async fn beat(&mut self) -> Result<(), CommonError> {
        self.sync_time_diff().await?;

        let request_packet = self.data.lock().await.build_uni_packet(
            UniCommand::OidbSvc,
            &self.data.lock().await.sig.hb480,
            None,
        )?;
        let request = self
            .request_sender
            .send_request(request_packet, None)
            .await?;
        match request.await {
            Ok(_) => {
                self.retried = 0;
                self.refresh_token().await?;

                Ok(())
            }
            Err(_) => {
                self.retried += 1;
                error!("heartbeat timeout, retried count: {}", self.retried);

                if self.retried >= 2 {
                    Err(CommonError::new("connection lost"))
                } else {
                    self.beat().await
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub token: Vec<u8>,
    pub nickname: String,
    pub gender: u8,
    pub age: u8,
}
#[derive(Debug)]
pub struct Data {
    pub pskey: HashMap<String, Vec<u8>>,
    pub uin: u32,
    pub apk: APK,
    pub device: FullDevice,
    pub sig: SIG,
    pub ecdh: ECDH,
}

impl Data {
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

impl Data {
    fn increase_seq(&mut self) -> u32 {
        let mut next = self.sig.seq + 1;
        if next >= 0x8000 {
            next = 0
        }

        self.sig.seq = next;
        next
    }

    fn build_common_packet<B>(
        &mut self,
        command: LoginCommand,
        body: B,
        r#type: Option<CommandType>,
    ) -> Result<LoginRequest, CommonError>
    where
        B: AsRef<[u8]>,
    {
        let seq = self.increase_seq();

        let r#type = r#type.unwrap_or(CommandType::Type2);
        let (uin, cmd_id, subid) = match command {
            LoginCommand::WtLoginTransEmp => (0, 0x812, Platform::watch().subid),
            _ => (self.uin, 0x810, self.apk.subid),
        };

        let mut buf_1;
        let body = match r#type {
            CommandType::Type2 => {
                let encrypted = tea::encrypt(&body, &self.ecdh.share_key)?;
                let mut buf_0 =
                    Vec::with_capacity(24 + self.ecdh.public_key.len() + encrypted.len());
                buf_0.write_u8(0x02)?;
                buf_0.write_u8(0x01)?;
                buf_0.write_all(&self.sig.randkey)?;
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

        let encrypted_sso = match r#type {
            CommandType::Type1 => tea::encrypt(sso, &self.sig.d2key)?,
            CommandType::Type2 => tea::encrypt(sso, &BUF_16)?,
            _ => sso,
        };

        let mut payload = Vec::with_capacity(100);
        payload.write_u32(0x0A)?;
        payload.write_u8(r#type as u8)?;
        payload.write_bytes_with_length(&self.sig.d2)?;
        payload.write_u8(0)?;
        payload.write_bytes_with_length(uin.to_string())?;
        payload.write_bytes(encrypted_sso)?;
        let mut result = Vec::with_capacity(payload.len() + 4);
        result.write_bytes_with_length(payload)?;

        Ok(LoginRequest::new(seq, command, result))
    }

    fn build_uni_packet<B>(
        &mut self,
        command: UniCommand,
        body: B,
        seq: Option<u32>,
    ) -> Result<UniRequest, CommonError>
    where
        B: AsRef<[u8]>,
    {
        let seq = seq.unwrap_or(self.increase_seq());

        let body = body.as_ref();
        let len = command.as_ref().len() + 20;
        let mut sso = Vec::with_capacity(len + body.len() + 4);
        sso.write_u32(len as u32)?;
        sso.write_u32((command.as_ref().len() + 4) as u32)?;
        sso.write_bytes(command.as_ref())?;
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
        payload.write_u32(0x08)?;
        payload.write_u8(1)?;
        payload.write_u32(seq)?;
        payload.write_u8(0)?;
        payload.write_u32((uin.len() + 4) as u32)?;
        payload.write_bytes(uin)?;
        payload.write_bytes(encrypt)?;

        Ok(UniRequest::new(seq, command, payload))
    }

    fn decode_t119<B>(&mut self, t119: B) -> Result<User, CommonError>
    where
        B: AsRef<[u8]>,
    {
        let decrypted = tea::decrypt(t119, &self.sig.tgtgt)?;
        let mut t = (&mut &decrypted[2..]).read_tlv()?;

        self.sig.tgt = t
            .remove(&0x10a)
            .ok_or(CommonError::new("tag 0x10a not existed"))?;
        self.sig.skey = t
            .remove(&0x120)
            .ok_or(CommonError::new("tag 0x120 not existed"))?;
        self.sig.d2 = t
            .remove(&0x143)
            .ok_or(CommonError::new("tag 0x143 not existed"))?;
        self.sig.d2key = t
            .remove(&0x305)
            .ok_or(CommonError::new("tag 0x305 not existed"))?[..16]
            .try_into()?;
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
        let ddd = t
            .remove(&0x11a)
            .ok_or(CommonError::new("tag 0x11a not existed"))?;
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

    fn decode_login_response<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<(), CommonError> {
        let decrypted = tea::decrypt(&mut &payload.as_ref()[16..], &self.ecdh.share_key)?;

        todo!()
    }
}
#[derive(Debug)]
struct Requester {
    network: Arc<Mutex<Network>>,
    polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Option<Vec<u8>>>>>>>,

    statistics: Arc<Mutex<Statistics>>,
}

impl Requester {
    fn new(
        statistics: Arc<Mutex<Statistics>>,
        network: Arc<Mutex<Network>>,
        polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Option<Vec<u8>>>>>>>,
    ) -> Self {
        Self {
            statistics,
            network,
            polling_requests,
        }
    }

    /// 等待返回结果
    pub async fn send_request<B>(
        &mut self,
        request_packet: B,
        timeout: Option<Duration>,
    ) -> Result<Response, CommonError>
    where
        B: Request,
    {
        let request = Response {
            timeout: timeout.unwrap_or(Duration::from_secs(5)),
            start: Instant::now(),
            response: Arc::new(Mutex::new(None)),
        };
        let response = Arc::downgrade(&request.response);
        self.polling_requests
            .lock()
            .await
            .insert(request_packet.seq(), response);
        self.write_request(request_packet).await?;

        Ok(request)
    }

    /// 不等待返回结果
    pub async fn write_request<B>(&mut self, request_packet: B) -> Result<(), CommonError>
    where
        B: Request,
    {
        self.network
            .lock()
            .await
            .send_bytes(request_packet.payload())
            .await?;

        self.statistics.lock().await.sent_pkt_cnt += 1;

        debug!(
            "send: {} seq: {}",
            request_packet.command(),
            request_packet.seq()
        );

        Ok(())
    }

    // /// 发送一个业务包但不等待返回
    // pub async fn write_uni(
    //     &mut self,
    //     request_packet: UniRequestPacket,
    //     seq: Option<u32>,
    // ) -> Result<(), CommonError> {
    //     self.network
    //         .lock()
    //         .await
    //         .send_bytes(request_packet.payload())
    //         .await?;

    //     self.data.lock().await.statistics.sent_pkt_cnt += 1;
    //     Ok(())
    // }

    // /// 发送一个业务包并等待返回结果
    // pub async fn send_uni<B: AsRef<[u8]>>(
    //     &mut self,
    //     command: UniCommand,
    //     body: B,
    //     timeout: Option<Duration>,
    // ) -> Result<Request, CommonError> {
    //     if self.is_online.load(Ordering::Relaxed) {
    //         let payload = self.data().await.build_uni_packet(command, body, None)?;
    //         let request = self
    //             .network
    //             .lock()
    //             .await
    //             .send_request(payload, timeout)
    //             .await?;
    //         self.data().await.statistics.sent_pkt_cnt += 1;
    //         Ok(request)
    //     } else {
    //         Err(CommonError::new("not online"))
    //     }
    // }

    // async fn send_login_request(&mut self, cmd: Command, body: &[u8]) {}
}

#[derive(Debug)]
pub struct BaseClient {
    data: Arc<Mutex<Data>>,
    statistics: Arc<Mutex<Statistics>>,

    registered: Arc<AtomicBool>,
    network: Arc<Mutex<Network>>,
    polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Option<Vec<u8>>>>>>>,
    heartbeat_handler: Option<JoinHandle<()>>,

    error_tx: Sender<InternalErrorKind>,
    sso_tx: Sender<(u32, String, Vec<u8>)>,
}

impl BaseClient {
    pub async fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        let statistics = Arc::new(Mutex::new(Statistics::new()));
        let mut instance = Self {
            data: Arc::new(Mutex::new(Data::new(uin, platform, d))),
            statistics,

            registered: Arc::new(AtomicBool::new(false)),
            network: Arc::new(Mutex::new(Network::new())),
            polling_requests: Arc::new(Mutex::new(HashMap::new())),
            heartbeat_handler: None,

            error_tx: sync::broadcast::channel(1).0,
            sso_tx: sync::broadcast::channel(1).0,
        };

        instance.describe_network_error().await;
        instance.describe_network_state().await;
        instance.describe_network_packet().await;

        instance
    }

    pub async fn default(uin: u32) -> Self {
        Self::new(uin, Platform::Android, None).await
    }
}

impl BaseClient {
    pub async fn data(&self) -> MutexGuard<Data> {
        self.data.lock().await
    }

    pub fn on_error(&self) -> Receiver<InternalErrorKind> {
        self.error_tx.subscribe()
    }

    pub fn on_sso(&self) -> Receiver<(u32, String, Vec<u8>)> {
        self.sso_tx.subscribe()
    }
}

impl BaseClient {
    async fn describe_network_error(&mut self) {
        let mut rx = self.network.lock().await.on_error();
        tokio::spawn(async move {
            while let Ok(err) = rx.recv().await {
                error!("{}", err.to_string());
            }
        });
    }

    async fn describe_network_state(&mut self) {
        let statistics = Arc::clone(&self.statistics);

        let mut rx = self.network.lock().await.on_state();
        tokio::spawn(async move {
            while let Ok((state, socket_addr)) = rx.recv().await {
                let socket_addr_str = socket_addr
                    .and_then(|socket_addr| Some(socket_addr.to_string()))
                    .unwrap_or("unknown remote server".to_string());

                let _ = match state {
                    NetworkState::Closed => {
                        statistics.lock().await.remote_socket_addr = None;

                        info!("{} closed", socket_addr_str);
                    }
                    NetworkState::Lost => {
                        statistics.lock().await.lost_times += 1;

                        error!("{} lost", socket_addr_str);
                    }
                    NetworkState::Connecting => {
                        info!("connecting...")
                    }
                    NetworkState::Connected => {
                        statistics.lock().await.remote_socket_addr = socket_addr.clone();

                        info!("{} connected", socket_addr_str);
                    }
                };
            }
        });
    }

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

    async fn describe_network_packet(&mut self) {
        let data = Arc::clone(&self.data);
        let statistics = Arc::clone(&self.statistics);
        let polling_requests = Arc::clone(&self.polling_requests);

        let mut rx = self.network.lock().await.on_packet();
        let error_tx = self.error_tx.clone();
        let sso_tx = self.sso_tx.clone();
        tokio::spawn(async move {
            let mut z_decompress = Decompress::new(true);
            while let Ok(packet) = rx.recv().await {
                statistics.lock().await.recv_pkt_cnt += 1;

                let flag = packet[4];
                let offset = u32::from_be_bytes(packet[6..10].try_into().unwrap());
                let encrypted = packet[offset as usize + 6..].to_vec();

                let decrypted = match flag {
                    0 => encrypted,
                    1 => match decrypt(&encrypted, &data.lock().await.sig.d2key) {
                        Ok(decrypted) => decrypted,
                        Err(err) => {
                            error!("tea decrypted error: {}", err);
                            continue;
                        }
                    },
                    2 => match decrypt(&encrypted, &BUF_16) {
                        Ok(decrypted) => decrypted,
                        Err(err) => {
                            error!("tea decrypted error: {}", err);
                            continue;
                        }
                    },
                    _ => {
                        let _ = error_tx.send(InternalErrorKind::Token);
                        error!("unknown flag: {}", flag);
                        continue;
                    }
                };

                match BaseClient::parse_sso(decrypted.as_slice(), &mut z_decompress) {
                    Ok(sso) => {
                        debug!("recv: {} seq: {}", sso.0, sso.1);

                        if let Some(packet) = polling_requests
                            .lock()
                            .await
                            .remove(&sso.0)
                            .and_then(|packet| packet.upgrade())
                        {
                            *packet.lock().await = Some(sso.2);
                        } else {
                            let _ = sso_tx.send((sso.0, sso.1, packet));
                        }
                    }
                    Err(err) => {
                        let _ = error!("sso parsec error: {}", err);
                    }
                };
            }
        });
    }
}

impl BaseClient {
    pub async fn register(
        &mut self,
        logout: Option<bool>,
        refresh: Option<bool>,
    ) -> Result<(), CommonError> {
        let logout = logout.unwrap_or(false);
        let refresh = refresh.unwrap_or(false);

        self.registered.store(false, Ordering::Relaxed);
        self.stop_heartbeat().await;

        let pb_buf = protobuf::encode(&ProtobufObject::from([(
            1,
            ProtobufElement::from([
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(46)),
                    (
                        2,
                        ProtobufElement::from(current_unix_timestamp_as_secs() as i64),
                    ),
                ])),
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(283)),
                    (2, ProtobufElement::from(0)),
                ])),
            ]),
        )]))?;

        let data = self.data().await;
        let d = &data.device;
        let svc_req_register = jce::encode(&JceObject::try_from([
            Some(JceElement::from(data.uin as i64)),
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
        drop(data);

        let pkt = self.data().await.build_common_packet(
            LoginCommand::StatSvcRegister,
            body,
            Some(CommandType::Type1),
        )?;

        let mut request_sender = Requester::from(self.as_ref());
        if logout {
            request_sender.write_request(pkt).await?;

            Ok(())
        } else {
            let request = request_sender
                .send_request(pkt, Some(Duration::from_secs(10)))
                .await?;
            let response = request.await?;

            let rsp = jce::decode_wrapper(&mut response.as_slice())?;
            if let JceElement::StructBegin(value) = rsp {
                let result = value
                    .get(&9)
                    .ok_or(CommonError::illegal_data())
                    .and_then(|e| {
                        if let JceElement::Int8(e) = e {
                            Ok(e)
                        } else {
                            Err(CommonError::illegal_data())
                        }
                    })
                    .and_then(|e| if *e != 0 { Ok(true) } else { Ok(false) })?;

                if !result && !refresh {
                    let _ = self.error_tx.send(InternalErrorKind::Token);
                } else {
                    self.registered.store(true, Ordering::Relaxed);
                    self.start_heartbeat().await;
                }

                Ok(())
            } else {
                Err(CommonError::illegal_data())
            }
        }
    }

    pub async fn connect(&mut self) -> Result<(), CommonError> {
        self.network.lock().await.connect().await
    }

    pub async fn disconnect(&mut self) -> Result<(), CommonError> {
        self.network.lock().await.disconnect().await
    }
}

impl BaseClient {
    async fn start_heartbeat(&mut self) {
        // 如果存在一个已有的异步线程，首先关闭
        self.stop_heartbeat().await;

        let mut time_synchronizer = Heartbeater::from(self.as_ref());
        let network = Arc::clone(&self.network);
        let registered = Arc::clone(&self.registered);
        self.heartbeat_handler = Some(tokio::spawn(async move {
            while registered.load(Ordering::Relaxed) {
                match time_synchronizer.beat().await {
                    Ok(_) => todo!(),
                    Err(_) => {
                        registered.store(false, Ordering::Relaxed);
                        let _ = network.lock().await.disconnect().await;
                        error!("heartbeat failure, disconnected.");
                        break;
                    }
                }
            }
        }));
    }

    async fn stop_heartbeat(&mut self) {
        if let Some(handler) = self.heartbeat_handler.take() {
            handler.abort();
            let _ = handler.await;
        }
    }
}

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

impl From<&BaseClient> for Requester {
    fn from(c: &BaseClient) -> Self {
        Self::new(
            Arc::clone(&c.statistics),
            Arc::clone(&c.network),
            Arc::clone(&c.polling_requests),
        )
    }
}

impl From<&BaseClient> for Heartbeater {
    fn from(c: &BaseClient) -> Self {
        Self::new(Arc::clone(&c.data), Requester::from(c))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use log::info;

    use crate::core::error::CommonError;

    use super::BaseClient;

    fn init_logger() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Trace)
            .is_test(true)
            .try_init();
    }

    #[tokio::test]
    async fn test() -> Result<(), CommonError> {
        init_logger();

        let mut base_client = BaseClient::default(1313).await;

        base_client.connect().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        base_client.disconnect().await?;
        drop(base_client);

        Ok(())
    }
}
