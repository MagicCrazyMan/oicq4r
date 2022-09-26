use std::cell::RefCell;

use log::Level;

use crate::define_observer;

use super::{
    device::{FullDevice, Platform, ShortDevice, APK},
    network::Network,
    writer::{write_bytes, write_i32, write_u32},
};

pub enum QrcodeResult {
    OtherError = 0x00,
    Timeout = 0x11,
    WaitingForScan = 0x30,
    WaitingForConfirm = 0x35,
    Canceled = 0x36,
}

pub struct BigData {
    pub ip: &'static str,
    pub port: u16,
    pub sig_session: [u8; 0],
    pub session_key: [u8; 0],
}

pub struct SIG {
    pub seq: u32,
    pub session: [u8; 4],
    pub randkey: [u8; 16],
    pub tgtgt: [u8; 16],
    pub tgt: [u8; 0],
    pub skey: [u8; 0],
    pub d2: [u8; 0],
    pub d2key: [u8; 0],
    pub t104: [u8; 0],
    pub t174: [u8; 0],
    pub qrsig: [u8; 0],
    pub bigdata: BigData,
    pub hb480: Vec<u8>,
    pub emp_time: u128,
    pub time_diff: u128,
}

impl SIG {
    pub fn new(
        seq: u32,
        session: [u8; 4],
        randkey: [u8; 16],
        tgtgt: [u8; 16],
        tgt: [u8; 0],
        skey: [u8; 0],
        d2: [u8; 0],
        d2key: [u8; 0],
        t104: [u8; 0],
        t174: [u8; 0],
        qrsig: [u8; 0],
        bigdata: BigData,
        hb480: Vec<u8>,
        emp_time: u128,
        time_diff: u128,
    ) -> Self {
        Self {
            seq,
            session,
            randkey,
            tgtgt,
            tgt,
            skey,
            d2,
            d2key,
            t104,
            t174,
            qrsig,
            bigdata,
            hb480,
            emp_time,
            time_diff,
        }
    }
}

define_observer! {
    BaseClientObserver,
    (internal_qrcode, QrcodeListener, (qrcode: &[u8; 16])),
    (internal_slider, SliderListener, (url: &str)),
    (internal_verify, VerifyListener, (url: &str, phone: &str)),
    (internal_error_token, ErrorTokenListener, ()),
    (internal_error_network, ErrorNetworkListener, (code: i64, message: &str)),
    (internal_error_login, ErrorLoginListener, (code: i64, message: &str)),
    (internal_error_qrcode, ErrorQrcodeListener, (code: &QrcodeResult, message: &str)),
    (internal_online, OnlineListener, (token: &[u8], nickname: &str, gender: u8, age: u8)),
    (internal_token, TokenListener, (token: &[u8])),
    (internal_kickoff, KickoffListener, (reason: &str)),
    (internal_sso, SsoListener, (cmd: &str, payload: &[u8], seq: i64)),
    (internal_verbose, VerboseListener, (verbose: &str, level: Level))
}

pub struct BaseClient {
    observer: BaseClientObserver,
    network: Network,
    uin: u32,
    apk: APK,
    device: FullDevice,
    sig: SIG,
}

impl BaseClient {
    // fn on_network_error(base_client: &mut BaseClient, code: i64, message: &str) {
    //     let a = A(base_client);
    //     base_client
    //         .observer()
    //         .internal_error_network
    //         .raise(&a, code, message)
    // }

    fn on_packet(&mut self, packet: &[u8]) {
        
    }
}

impl BaseClient {
    pub fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        let observer = BaseClientObserver::new();
        let apk = platform.metadata();
        let device = FullDevice::from(d.unwrap_or(ShortDevice::generate(uin)));
        let network = Network::new();
        // let sig = SIG::ne

        let mut instance = Self {
            observer,
            network,
            uin,
            apk,
            device,
            sig: todo!(),
        };
        instance.init_verbose();
        instance.init_packet();

        instance
    }

    pub fn default(uin: u32) -> Self {
        Self::new(uin, Platform::Android, None)
    }

    fn init_verbose(&mut self) {
        let p = self as *mut BaseClient;

        self.network.observer().error.on(
            move |err| unsafe {
                let base_client = &mut *p;
                base_client
                    .observer
                    .internal_verbose
                    .raise(err, Level::Error);
            },
            false,
        );

        self.network.observer().closed.on(
            move |server| unsafe {
                let base_client = &mut *p;

                // let
                base_client.observer.internal_verbose.raise(
                    format!("{}:{} closed", server.ip(), server.port()).as_str(),
                    Level::Error,
                );
            },
            false,
        );

        self.network.observer().connected.on(
            move |server| unsafe {
                let base_client = &mut *p;

                // let
                base_client.observer.internal_verbose.raise(
                    format!("{}:{} connected", server.0, server.1).as_str(),
                    Level::Error,
                );
            },
            false,
        );
    }

    fn init_packet(&mut self) {
        let p = self as *mut BaseClient;

        self.network.observer().packet.on(
            move |packet| unsafe {
                (&mut *p).on_packet(packet);
            },
            false,
        );
    }

    pub fn observer(&mut self) -> &mut BaseClientObserver {
        &mut self.observer
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

    pub fn sig(&self) -> &SIG {
        &self.sig
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
