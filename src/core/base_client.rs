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

trait RequestPacket {
    fn seq(&self) -> u32;

    fn command(&self) -> &str;

    fn payload(&self) -> &[u8];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    WtLoginLogin,
    WtLoginExchangeEmp,
    WtLoginTransEmp,
    StatSvcRegister,
    ClientCorrectTime,
}

impl AsRef<str> for Command {
    fn as_ref(&self) -> &str {
        match self {
            Command::WtLoginLogin => "wtlogin.login",
            Command::WtLoginExchangeEmp => "wtlogin.exchange_emp",
            Command::WtLoginTransEmp => "wtlogin.trans_emp",
            Command::StatSvcRegister => "StatSvc.register",
            Command::ClientCorrectTime => "Client.CorrectTime",
        }
    }
}

struct CommonRequestPacket {
    seq: u32,
    command: Command,
    payload: Vec<u8>,
}

impl CommonRequestPacket {
    fn new(seq: u32, command: Command, payload: Vec<u8>) -> Self {
        Self {
            seq,
            command,
            payload,
        }
    }
}

impl RequestPacket for CommonRequestPacket {
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

struct UniRequestPacket {
    seq: u32,
    command: UniCommand,
    payload: Vec<u8>,
}

impl UniRequestPacket {
    fn new(seq: u32, command: UniCommand, payload: Vec<u8>) -> Self {
        Self {
            seq,
            command,
            payload,
        }
    }
}

impl RequestPacket for UniRequestPacket {
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
pub struct User {
    pub token: Vec<u8>,
    pub nickname: String,
    pub gender: u8,
    pub age: u8,
}

#[derive(Debug)]
pub struct BaseClientData {
    pub statistics: Statistics,
    pub pskey: HashMap<String, Vec<u8>>,
    pub uin: u32,
    pub apk: APK,
    pub device: FullDevice,
    pub sig: SIG,
    pub ecdh: ECDH,
}

impl BaseClientData {
    fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        Self {
            statistics: Statistics::new(),
            pskey: HashMap::new(),
            uin,
            apk: platform.metadata(),
            device: FullDevice::from(d.unwrap_or(ShortDevice::generate(uin))),
            sig: SIG::new(uin),
            ecdh: ECDH::new(),
        }
    }
}

impl BaseClientData {
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
        command: Command,
        body: B,
        r#type: Option<CommandType>,
    ) -> Result<CommonRequestPacket, CommonError>
    where
        B: AsRef<[u8]>,
    {
        let seq = self.increase_seq();

        let r#type = r#type.unwrap_or(CommandType::Type2);
        let (uin, cmd_id, subid) = match command {
            Command::WtLoginTransEmp => (0, 0x812, Platform::watch().subid),
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

        Ok(CommonRequestPacket::new(seq, command, result))
    }

    fn build_uni_packet<B>(
        &mut self,
        command: UniCommand,
        body: B,
        seq: Option<u32>,
    ) -> Result<UniRequestPacket, CommonError>
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

        Ok(UniRequestPacket::new(seq, command, payload))
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

    fn decode_login_response<B: AsRef<[u8]>>(&mut self, payload: B ) -> Result<(), CommonError> {
        let decrypted = tea::decrypt(&mut &payload.as_ref()[16..], &self.ecdh.share_key)?;

        let r#type = decrypted[2];
        todo!()
    }
}

#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Request {
    timeout: Duration,
    start: Instant,
    response: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Future for Request {
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
struct RequestSender {
    data: Arc<Mutex<BaseClientData>>,
    network: Arc<Mutex<Network>>,
    polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Option<Vec<u8>>>>>>>,
    verbose_tx: Sender<(String, Level)>,
}

impl RequestSender {
    fn new(
        data: Arc<Mutex<BaseClientData>>,
        network: Arc<Mutex<Network>>,
        polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Option<Vec<u8>>>>>>>,
        verbose_tx: Sender<(String, Level)>,
    ) -> Self {
        Self {
            data,
            network,
            polling_requests,
            verbose_tx,
        }
    }

    /// 等待返回结果
    pub async fn send_request<B>(
        &mut self,
        request_packet: B,
        timeout: Option<Duration>,
    ) -> Result<Request, CommonError>
    where
        B: RequestPacket,
    {
        let request = Request {
            timeout: timeout.unwrap_or(Duration::from_secs(5)),
            start: Instant::now(),
            response: Arc::new(Mutex::new(None)),
        };
        let response = Arc::downgrade(&request.response);
        self.polling_requests
            .lock()
            .await
            .insert(request_packet.seq(), response);
        self.network
            .lock()
            .await
            .send_bytes(request_packet.payload())
            .await?;

        self.data.lock().await.statistics.sent_pkt_cnt += 1;

        let _ = self.verbose_tx.send((
            format!(
                "send: {} seq: {}",
                request_packet.command(),
                request_packet.seq()
            ),
            Level::Debug,
        ));

        Ok(request)
    }

    /// 不等待返回结果
    pub async fn write_request<B>(&mut self, request_packet: B) -> Result<(), CommonError>
    where
        B: RequestPacket,
    {
        self.network
            .lock()
            .await
            .send_bytes(request_packet.payload())
            .await?;

        self.data.lock().await.statistics.sent_pkt_cnt += 1;

        let _ = self.verbose_tx.send((
            format!(
                "send: {} seq: {}",
                request_packet.command(),
                request_packet.seq()
            ),
            Level::Debug,
        ));

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
struct Heartbeater {
    retried: u8,
    data: Arc<Mutex<BaseClientData>>,
    request_sender: RequestSender,
    verbose_tx: Sender<(String, Level)>,
    token_tx: Sender<Vec<u8>>,
}

impl Heartbeater {
    fn new(
        data: Arc<Mutex<BaseClientData>>,
        request_sender: RequestSender,
        verbose_tx: Sender<(String, Level)>,
        token_tx: Sender<Vec<u8>>,
    ) -> Self {
        Self {
            retried: 0,
            data,
            request_sender,
            verbose_tx,
            token_tx,
        }
    }

    async fn sync_time_diff(&mut self) -> Result<(), CommonError> {
        let request_packet = self.data.lock().await.build_common_packet(
            Command::ClientCorrectTime,
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
        let packet = data.build_common_packet(Command::WtLoginExchangeEmp, body, None)?;
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
            
            self.token_tx.send(user.token);

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
                let _ = self.verbose_tx.send((
                    format!("heartbeat timeout, retried count: {}", self.retried),
                    Level::Error,
                ));

                if self.retried >= 2 {
                    Err(CommonError::new("connection lost"))
                } else {
                    self.beat().await
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct BaseClient {
    data: Arc<Mutex<BaseClientData>>,

    registered: Arc<AtomicBool>,
    network: Arc<Mutex<Network>>,
    polling_requests: Arc<Mutex<HashMap<u32, Weak<Mutex<Option<Vec<u8>>>>>>>,
    heartbeat_handler: Option<JoinHandle<()>>,

    verbose_tx: Sender<(String, Level)>,
    error_tx: Sender<InternalErrorKind>,
    sso_tx: Sender<(u32, String, Vec<u8>)>,
    token_tx: Sender<String>,
}

impl BaseClient {
    pub async fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        let mut instance = Self {
            data: Arc::new(Mutex::new(BaseClientData::new(uin, platform, d))),

            registered: Arc::new(AtomicBool::new(false)),
            network: Arc::new(Mutex::new(Network::new())),
            polling_requests: Arc::new(Mutex::new(HashMap::new())),
            heartbeat_handler: None,

            verbose_tx: sync::broadcast::channel(1).0,
            error_tx: sync::broadcast::channel(1).0,
            sso_tx: sync::broadcast::channel(1).0,
            token_tx: sync::broadcast::channel(1).0,
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
    pub async fn data(&self) -> MutexGuard<BaseClientData> {
        self.data.lock().await
    }

    pub fn on_verbose(&self) -> Receiver<(String, Level)> {
        self.verbose_tx.subscribe()
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
        let verbose_tx = self.verbose_tx.clone();

        let mut rx = self.network.lock().await.on_error();
        tokio::spawn(async move {
            while let Ok(err) = rx.recv().await {
                let _ = verbose_tx.send((err.to_string(), Level::Error));
            }
        });
    }

    async fn describe_network_state(&mut self) {
        let data = Arc::clone(&self.data);
        let verbose_tx = self.verbose_tx.clone();

        let mut rx = self.network.lock().await.on_state();
        tokio::spawn(async move {
            while let Ok((state, socket_addr)) = rx.recv().await {
                let _ = match state {
                    NetworkState::Closed => {
                        data.lock().await.statistics.remote_socket_addr = None;

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
                        data.lock().await.statistics.lost_times += 1;

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
                        data.lock().await.statistics.remote_socket_addr = socket_addr.clone();

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
        let polling_requests = Arc::clone(&self.polling_requests);

        let mut rx = self.network.lock().await.on_packet();
        let error_tx = self.error_tx.clone();
        let verbose_tx = self.verbose_tx.clone();
        let sso_tx = self.sso_tx.clone();
        tokio::spawn(async move {
            let mut z_decompress = Decompress::new(true);
            while let Ok(packet) = rx.recv().await {
                data.lock().await.statistics.recv_pkt_cnt += 1;

                let flag = packet[4];
                let offset = u32::from_be_bytes(packet[6..10].try_into().unwrap());
                let encrypted = packet[offset as usize + 6..].to_vec();

                let decrypted = match flag {
                    0 => encrypted,
                    1 => match decrypt(&encrypted, &data.lock().await.sig.d2key) {
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
                        let _ =
                            verbose_tx.send((format!("sso parsec error: {}", err), Level::Error));
                    }
                };
            }
        });
    }
}

impl BaseClient {pub async fn register(
    &mut self,
    logout: Option<bool>,
    refresh: Option<bool>,
) -> Result<(), CommonError> { 

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
        let verbose_tx = self.verbose_tx.clone();
        self.heartbeat_handler = Some(tokio::spawn(async move {
            while registered.load(Ordering::Relaxed) {
                match time_synchronizer.beat().await {
                    Ok(_) => todo!(),
                    Err(_) => {
                        registered.store(false, Ordering::Relaxed);
                        let _ = network.lock().await.disconnect().await;
                        let _ = verbose_tx
                            .send(("heartbeat failure, disconnected.".to_string(), Level::Error));
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

impl From<&BaseClient> for RequestSender {
    fn from(c: &BaseClient) -> Self {
        Self::new(
            Arc::clone(&c.data),
            Arc::clone(&c.network),
            Arc::clone(&c.polling_requests),
            c.verbose_tx.clone(),
        )
    }
}

impl From<&BaseClient> for Heartbeater {
    fn from(c: &BaseClient) -> Self {
        Self::new(
            Arc::clone(&c.data),
            RequestSender::from(c),
            c.verbose_tx.clone(),
        )
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::core::error::CommonError;

    use super::BaseClient;

    #[tokio::test]
    async fn test() -> Result<(), CommonError> {
        let mut base_client = BaseClient::default(1313).await;

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

        base_client.connect().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        base_client.disconnect().await?;
        drop(base_client);
        handler.await?;

        Ok(())
    }
}
