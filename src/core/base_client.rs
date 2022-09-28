use std::{borrow::BorrowMut, cell::RefCell, rc::Rc};

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

define_observer! {
    Observer,
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

struct BaseClientWrapped {
    uin: u32,
    apk: APK,
    device: FullDevice,
    sig: SIG,
    observer: Observer,
    network: Network,
}

impl BaseClientWrapped {
    fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        Self {
            uin,
            apk: platform.metadata(),
            device: FullDevice::from(d.unwrap_or(ShortDevice::generate(uin))),
            sig: todo!(),
            network: Network::new(),
            observer: Observer::new(),
        }
    }
}

pub struct BaseClient(Rc<RefCell<BaseClientWrapped>>);

impl BaseClient {
    pub fn new(uin: u32, platform: Platform, d: Option<ShortDevice>) -> Self {
        let instance = Self(Rc::new(RefCell::new(BaseClientWrapped::new(
            uin, platform, d,
        ))));
        instance.register_verbose();
        // instance.init_packet();

        instance
    }

    pub fn default(uin: u32) -> Self {
        Self::new(uin, Platform::Android, None)
    }

    fn register_verbose(&self) {
        let mut m = (*self.0).borrow_mut();

        let a = Rc::clone(&self.0);
        m.network.observer.error.on(
            move |err| {
                let mut b = (*a).borrow_mut();
                b.borrow_mut()
                    .observer
                    .internal_verbose
                    .raise(err, Level::Error);
            },
            false,
        );

        let a = Rc::clone(&self.0);
        m.network.observer.closed.on(
            move |server| {
                let mut b = (*a).borrow_mut();
                b.observer.internal_verbose.raise(
                    format!("{}:{} closed", server.ip(), server.port()).as_str(),
                    Level::Error,
                );
            },
            false,
        );

        let a = Rc::clone(&self.0);
        m.network.observer.connected.on(
            move |server| {
                let mut b = (*a).borrow_mut();
                b.observer.internal_verbose.raise(
                    format!("{}:{} connected", server.ip(), server.port()).as_str(),
                    Level::Error,
                );
            },
            false,
        );
    }

    // fn init_packet(&mut self) {
    //     self.network.observer.packet.on(
    //         |packet| {
    //             self.data.borrow_mut().apk.appid = 0;
    //         },
    //         false,
    //     );
    // }

    // pub fn observer(&mut self) -> &mut BaseClientObserver {
    //     &mut self.observer
    // }
}

impl BaseClient {
    pub fn uin(&self) -> u32 {
        (*self.0).borrow().uin
    }

    pub fn device(&self) -> &FullDevice {
        &(*self.0).borrow().device
    }

    pub fn apk(&self) -> APK {
        (*self.0).borrow().apk
    }

    pub fn sig(&self) -> &SIG {
        &(*self.0).borrow().sig
    }
}

impl AsRef<BaseClientWrapped> for BaseClientWrapped {
    fn as_ref(&self) -> &BaseClientWrapped {
        self
    }
}

impl AsMut<BaseClientWrapped> for BaseClientWrapped {
    fn as_mut(&mut self) -> &mut BaseClientWrapped {
        self
    }
}
