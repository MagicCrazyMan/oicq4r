#[derive(Debug, Clone, Copy)]
pub struct APK {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub ver: &'static str,
    pub sign: [u8; 16],
    pub buildtime: u32,
    pub appid: u32,
    pub subid: u32,
    pub bitmap: u32,
    pub sigmap: u32,
    pub sdkver: &'static str,
    pub display: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub enum Platform {
    Android,
    AndroidPad,
    Watch,
    IMac,
    IPad,
}

impl Platform {
    pub fn metadata(&self) -> APK {
        match self {
            Platform::Android => Platform::mobile(),
            Platform::AndroidPad => Platform::android_pad(),
            Platform::Watch => Platform::watch(),
            Platform::IMac => Platform::i_mac(),
            Platform::IPad => Platform::i_pad(),
        }
    }

    pub fn mobile() -> APK {
        APK {
            id: "com.tencent.mobileqq",
            name: "A8.8.80.7400",
            version: "8.8.80.7400",
            ver: "8.8.80",
            sign: [
                0xa6, 0xb7, 0x45, 0xbf, 0x24, 0xa2, 0xc2, 0x77, 0x52, 0x77, 0x16, 0xf6, 0xf3, 0x6e,
                0xb6, 0x8d,
            ],
            buildtime: 1640921786,
            appid: 16,
            subid: 537113159,
            bitmap: 184024956,
            sigmap: 34869472,
            sdkver: "6.0.0.2494",
            display: "Android",
        }
    }

    pub fn watch() -> APK {
        APK {
            id: "com.tencent.qqlite",
            name: "A2.0.5",
            version: "2.0.5",
            ver: "2.0.5",
            sign: [
                0xa6, 0xb7, 0x45, 0xbf, 0x24, 0xa2, 0xc2, 0x77, 0x52, 0x77, 0x16, 0xf6, 0xf3, 0x6e,
                0xb6, 0x8d,
            ],
            buildtime: 1559564731,
            appid: 16,
            subid: 537064446,
            bitmap: 16252796,
            sigmap: 34869472,
            sdkver: "6.0.0.236",
            display: "Watch",
        }
    }

    pub fn android_pad() -> APK {
        APK {
            id: "com.tencent.minihd.qq",
            name: "A5.9.3.3468",
            version: "5.9.3.3468",
            ver: "5.9.3",
            sign: [
                0xaa, 0x39, 0x78, 0xf4, 0x1f, 0xd9, 0x6f, 0xf9, 0x91, 0x4a, 0x66, 0x9e, 0x18, 0x64,
                0x74, 0xc7,
            ],
            buildtime: 1637427966,
            appid: 16,
            subid: 537067382,
            bitmap: 150470524,
            sigmap: 1970400,
            sdkver: "6.0.0.2487",
            display: "aPad",
        }
    }

    pub fn i_mac() -> APK {
        APK {
            id: "com.tencent.minihd.qq",
            name: "A5.9.3.3468",
            version: "5.9.3.3468",
            ver: "5.9.3",
            sign: [
                0xaa, 0x39, 0x78, 0xf4, 0x1f, 0xd9, 0x6f, 0xf9, 0x91, 0x4a, 0x66, 0x9e, 0x18, 0x64,
                0x74, 0xc7,
            ],
            buildtime: 1637427966,
            appid: 16,
            subid: 537128930,
            bitmap: 150470524,
            sigmap: 1970400,
            sdkver: "6.0.0.2487",
            display: "iMac",
        }
    }

    pub fn i_pad() -> APK {
        APK {
            id: "com.tencent.minihd.qq",
            name: "A5.9.3.3468",
            version: "5.9.3.3468",
            ver: "5.9.3",
            sign: [
                0xaa, 0x39, 0x78, 0xf4, 0x1f, 0xd9, 0x6f, 0xf9, 0x91, 0x4a, 0x66, 0x9e, 0x18, 0x64,
                0x74, 0xc7,
            ],
            buildtime: 1637427966,
            appid: 16,
            subid: 537118796,
            bitmap: 150470524,
            sigmap: 1970400,
            sdkver: "6.0.0.2487",
            display: "iPad",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShortDevice {
    pub product: &'static str,
    pub device: &'static str,
    pub board: &'static str,
    pub brand: &'static str,
    pub model: &'static str,
    pub wifi_ssid: String,
    pub bootloader: &'static str,
    pub android_id: String,
    pub boot_id: String,
    pub proc_version: String,
    pub mac_address: String,
    pub ip_address: String,
    pub imei: String,
    pub incremental: u32,
}

impl ShortDevice {
    pub fn generate(uin: u32) -> ShortDevice {
        let hash = md5::compute(uin.to_string());
        let hex = format!("{:x}", hash);

        let wifi_ssid = format!("TP-LINK-{:x}", &uin);
        let android_id = format!(
            "OICQX.{}{}.{}{}",
            u16::from_be_bytes([hash[0], hash[1]]),
            hash[2].to_string(),
            hash[3].to_string(),
            uin.to_string().chars().nth(0).unwrap(),
        );
        let boot_id = format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        );
        let proc_version = format!(
            "Linux version 4.19.71-{}",
            u16::from_be_bytes([hash[4], hash[5]])
        );
        let mac_address = format!(
            "00:50:{:2X}:{:2X}:{:2X}:{:2X}",
            hash[6], hash[7], hash[8], hash[9]
        );
        let ip_address = format!("10.0.{}.{}", hash[10], hash[11]);
        let imei = ShortDevice::generate_imei(uin);
        let incremental = u32::from_be_bytes([hash[12], hash[13], hash[14], hash[15]]);

        ShortDevice {
            product: "MRS4S",
            device: "HIM188MOE",
            board: "MIRAI-YYDS",
            brand: "OICQX",
            model: "Konata 2020",
            wifi_ssid,
            bootloader: "U-boot",
            android_id,
            boot_id,
            proc_version,
            mac_address,
            ip_address,
            imei,
            incremental,
        }
    }

    fn generate_imei(uin: u32) -> String {
        let buf = uin.to_be_bytes();

        let a = u16::from_be_bytes([buf[0], buf[1]]);
        let mut b = u32::from_be_bytes([0, buf[1], buf[2], buf[3]]);

        let uin_str = uin.to_string();
        let a_str = if a > 9999 {
            (a / 10).to_string()
        } else if a < 1000 {
            (&uin_str[0..4]).to_string()
        } else {
            a.to_string()
        };

        while b > 9999999 {
            b >>= 1;
        }
        let b_str = if b < 1000000 {
            format!("{}{}", &uin_str[0..4], &uin_str[0..3])
        } else {
            b.to_string()
        };

        let imei = format!(
            "{}{}0{}",
            if uin % 2 == 0 { "35" } else { "86" },
            &a_str,
            &b_str
        );

        let sp = {
            let mut sum = 0;
            for (i, char) in imei.char_indices() {
                if i % 2 == 0 {
                    sum += char.to_digit(10).unwrap();
                } else {
                    let j = char.to_digit(10).unwrap() * 2;
                    sum += j % 10 + (j as f32 / 10.0).floor() as u32;
                }
            }
            (100 - sum) % 10
        };

        format!("{}{}", imei, sp)
    }
}

#[derive(Debug, Clone)]
pub struct Version {
    pub incremental: u32,
    pub release: &'static str,
    pub codename: &'static str,
    pub sdk: u8,
}

#[derive(Debug, Clone)]
pub struct FullDevice {
    pub product: &'static str,
    pub device: &'static str,
    pub board: &'static str,
    pub brand: &'static str,
    pub model: &'static str,
    pub wifi_ssid: String,
    pub bootloader: &'static str,
    pub android_id: String,
    pub boot_id: String,
    pub proc_version: String,
    pub mac_address: String,
    pub ip_address: String,
    pub imei: String,

    pub display: String,
    pub fingerprint: String,
    pub baseband: &'static str,
    pub sim: &'static str,
    pub os_type: &'static str,
    pub wifi_bssid: String,
    pub apn: &'static str,
    pub version: Version,
    pub imsi: [u8; 16],
    pub guid: [u8; 16],
}

impl From<ShortDevice> for FullDevice {
    fn from(d: ShortDevice) -> Self {
        let version = Version {
            incremental: d.incremental,
            release: "10",
            codename: "REL",
            sdk: 29,
        };

        let imsi = rand::random::<[u8; 16]>();
        let mut raw_guid: Vec<u8> =
            Vec::with_capacity(d.imei.as_bytes().len() + d.mac_address.as_bytes().len());
        raw_guid.extend(d.imei.as_bytes());
        raw_guid.extend(d.mac_address.as_bytes());
        let guid = md5::compute(raw_guid).0;
        Self {
            display: d.android_id.clone(),
            product: d.product,
            device: d.device,
            board: d.board,
            brand: d.brand,
            model: d.model,
            bootloader: d.bootloader,
            fingerprint: format!(
                "{}/{}/{}:10/{}/{}:user/release-keys",
                d.brand, d.product, d.device, d.android_id, d.incremental
            ),
            boot_id: d.boot_id,
            proc_version: d.proc_version,
            baseband: "",
            sim: "T-Mobile",
            os_type: "android",
            mac_address: d.mac_address.clone(),
            ip_address: d.ip_address,
            wifi_bssid: d.mac_address,
            wifi_ssid: d.wifi_ssid,
            imei: d.imei,
            android_id: d.android_id,
            apn: "wifi",
            version,
            imsi,
            guid,
        }
    }
}
