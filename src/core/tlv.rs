use std::{
    collections::HashMap,
    fmt::Display,
    io::{Read, Write},
};

use crate::error::Error;

use super::{
    base_client::DataCenter,
    device::Platform,
    helper::{current_unix_timestamp_as_millis, BUF_0, BUF_1, BUF_4},
    io::WriteExt,
    protobuf::{encode, ProtobufElement, ProtobufObject},
    tea::{self, encrypt},
};

#[derive(Debug)]
pub enum TlvError {
    InvalidData,
    PasswordNotProvided,
    CodeNotProvided,
    TicketNotProvided,
    TagNotExisted(i32),
}

impl std::error::Error for TlvError {}

impl Display for TlvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlvError::PasswordNotProvided => f.write_str("password not provided."),
            TlvError::CodeNotProvided => f.write_str("code not provided."),
            TlvError::TicketNotProvided => f.write_str("ticket not provided."),
            TlvError::InvalidData => f.write_str("invalidate data."),
            TlvError::TagNotExisted(tag) => {
                f.write_fmt(format_args!("tag 0x:{:x} not existed", tag))
            }
        }
    }
}

trait MaximumSlice {
    fn partial_slice(&self, max: usize) -> &str;
}

impl<T: AsRef<str>> MaximumSlice for T {
    fn partial_slice(&self, max: usize) -> &str {
        let str = self.as_ref();
        let len = if str.len() > max { max } else { str.len() };
        &str[..len]
    }
}

fn pack_body(
    data: &DataCenter,
    tag: u16,
    emp: Option<u32>,
    md5_password: Option<[u8; 16]>,
    code: Option<Vec<u8>>,
    ticket: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let result = match tag {
        0x01 => {
            let mut writer = Vec::with_capacity(2 + 4 + 4 + 4 + 4 + 2);
            writer.write_u16(1)?;
            writer.write_bytes(&rand::random::<[u8; 4]>())?;
            writer.write_u32(data.uin)?;
            writer.write_u32(current_unix_timestamp_as_millis() as u32)?;
            writer.write_bytes(BUF_4)?;
            writer.write_u16(0)?;
            writer
        }
        0x08 => {
            let mut writer = Vec::with_capacity(2 + 4 + 2);
            writer.write_u16(0)?;
            writer.write_u32(2052)?;
            writer.write_u16(0)?;
            writer
        }
        0x16 => {
            let apk = Platform::watch();
            let mut writer = Vec::with_capacity(
                4 + 4
                    + 4
                    + data.device.guid.len()
                    + 2
                    + apk.id.len()
                    + 2
                    + apk.ver.len()
                    + 2
                    + apk.sign.len(),
            );
            writer.write_u32(7)?;
            writer.write_u32(apk.appid)?;
            writer.write_u32(apk.subid)?;
            writer.write_bytes(&data.device.guid)?;
            writer.write_tlv(apk.id)?;
            writer.write_tlv(apk.ver)?;
            writer.write_tlv(apk.sign)?;
            writer
        }
        0x18 => {
            let mut writer = Vec::with_capacity(2 + 4 + 4 + 4 + 4 + 2 + 2);
            writer.write_u16(1)?;
            writer.write_u32(1536)?;
            writer.write_u32(data.apk.appid)?;
            writer.write_u32(0)?;
            writer.write_u32(data.uin)?;
            writer.write_u16(0)?;
            writer.write_u16(0)?;
            writer
        }
        0x1B => {
            let mut writer = Vec::with_capacity(4 * 7 + 2);
            writer.write_u32(0)?;
            writer.write_u32(0)?;
            writer.write_u32(3)?;
            writer.write_u32(4)?;
            writer.write_u32(72)?;
            writer.write_u32(2)?;
            writer.write_u32(2)?;
            writer.write_u16(0)?;
            writer
        }
        0x1D => {
            let mut writer = Vec::with_capacity(1 + 4 + 4 + 1 + 4);
            writer.write_u8(1)?;
            writer.write_u32(184024956)?;
            writer.write_u32(0)?;
            writer.write_u8(0)?;
            writer.write_u32(0)?;
            writer
        }
        0x1F => {
            let mut writer = Vec::with_capacity(1 + 2 + 7 + 2 + 5 + 2 + 2 + 16 + 2 + 0 + 2 + 4);
            writer.write_u8(0)?;
            writer.write_tlv("android")?;
            writer.write_tlv("7.1.2")?;
            writer.write_u16(2)?;
            writer.write_tlv("China Mobile GSM")?;
            writer.write_tlv(BUF_0)?;
            writer.write_tlv("wifi")?;
            writer
        }
        0x33 => data.device.guid.to_vec(),
        0x35 => 8u32.to_be_bytes().to_vec(),
        0x100 => {
            let mut writer = Vec::with_capacity(2 + 5 * 4);
            writer.write_u16(1)?;
            writer.write_u32(7)?;
            writer.write_u32(data.apk.appid)?;
            writer.write_u32(if emp.is_some() { 2 } else { data.apk.subid })?;
            writer.write_u32(0)?;
            writer.write_u32(data.apk.sigmap)?;
            writer
        }
        0x104 => data.sig.t104.clone(),
        0x106 => {
            let md5_password = md5_password.ok_or(TlvError::PasswordNotProvided)?;
            let mut body = Vec::with_capacity(
                2 + 4 + 4 + 4 + 4 + 8 + 4 + 4 + 1 + 16 + 16 + 4 + 1 + 16 + 4 + 4 + 2 + 4 + 2 + 24,
            );
            body.write_u16(4)?;
            body.write_bytes(rand::random::<[u8; 4]>())?;
            body.write_u32(7)?;
            body.write_u32(data.apk.appid)?;
            body.write_u32(0)?;
            body.write_u64(data.uin as u64)?;
            body.write_u32(current_unix_timestamp_as_millis() as u32)?;
            body.write_bytes([0; 4])?;
            body.write_u8(1)?;
            body.write_bytes(&md5_password)?;
            body.write_bytes(data.sig.tgtgt)?;
            body.write_u32(0)?;
            body.write_u8(1)?;
            body.write_bytes(data.device.guid)?;
            body.write_u32(data.apk.subid)?;
            body.write_u32(1)?;
            body.write_tlv(data.uin.to_string())?;
            body.write_u16(0)?;

            let mut key = Vec::with_capacity(16 + 4 + 4);
            key.extend(md5_password);
            key.extend([0; 4]);
            key.extend(data.uin.to_be_bytes());
            let key = md5::compute(&key).0;

            body.extend(key);
            let encrypted = tea::encrypt(body, &key)?;

            let mut writer = Vec::with_capacity(encrypted.len());
            writer.write_bytes(encrypted)?;
            writer
        }
        0x107 => {
            let mut writer = Vec::with_capacity(2 + 1 + 2 + 1);
            writer.write_u16(0)?;
            writer.write_u8(0)?;
            writer.write_u16(0)?;
            writer.write_u8(1)?;
            writer
        }
        0x109 => md5::compute(&data.device.imei).0.to_vec(),
        0x10a => data.sig.tgt.clone(),
        0x116 => {
            let mut writer = Vec::with_capacity(1 + 4 + 4 + 1 + 4);
            writer.write_u8(0)?;
            writer.write_u32(data.apk.bitmap)?;
            writer.write_u32(0x10400)?;
            writer.write_u8(1)?;
            writer.write_u32(1600000226)?;
            writer
        }
        0x124 => {
            let mut writer = Vec::with_capacity(2 + 16 + 2 + 16 + 2 + 2 + 16 + 2 + 2 + 16);
            writer.write_tlv(&data.device.os_type.partial_slice(16))?;
            writer.write_tlv(&data.device.version.release.partial_slice(16))?;
            writer.write_u16(2)?;
            writer.write_tlv(&data.device.sim.partial_slice(16))?;
            writer.write_u16(0)?;
            writer.write_tlv(&data.device.apn.partial_slice(16))?;
            writer
        }
        0x128 => {
            let mut writer = Vec::with_capacity(2 + 1 + 1 + 1 + 4 + 2 + 32 + 2 + 16 + 2 + 16);
            writer.write_u16(0)?;
            writer.write_u8(0)?;
            writer.write_u8(1)?;
            writer.write_u8(0)?;
            writer.write_u32(16777216)?;
            writer.write_tlv(&data.device.model.partial_slice(32))?;
            writer.write_tlv(&data.device.guid)?;
            writer.write_tlv(&data.device.brand.partial_slice(16))?;
            writer
        }
        0x141 => {
            let mut writer =
                Vec::with_capacity(2 + 2 + data.device.sim.len() + 2 + 2 + data.device.apn.len());
            writer.write_u16(1)?;
            writer.write_tlv(data.device.sim)?;
            writer.write_u16(2)?;
            writer.write_tlv(data.device.apn)?;
            writer
        }
        0x142 => {
            let mut writer = Vec::with_capacity(2 + 2 + 32);
            writer.write_u16(0)?;
            writer.write_tlv(&data.apk.id.partial_slice(32))?;
            writer
        }
        0x143 => data.sig.d2.clone(),
        0x144 => {
            let a = pack(data, 0x109)?;
            let b = pack(data, 0x52d)?;
            let c = pack(data, 0x124)?;
            let d = pack(data, 0x128)?;
            let e = pack(data, 0x16e)?;
            let mut body = Vec::with_capacity(2 + a.len() + b.len() + c.len() + d.len() + e.len());
            body.write_u16(5)?;
            body.write_bytes(a)?;
            body.write_bytes(b)?;
            body.write_bytes(c)?;
            body.write_bytes(d)?;
            body.write_bytes(e)?;

            encrypt(body, &data.sig.tgtgt)?
        }
        0x145 => data.device.guid.to_vec(),
        0x147 => {
            let mut writer = Vec::with_capacity(4 + 2 + 5 + 2 + data.apk.sign.len());
            writer.write_u32(data.apk.appid)?;
            writer.write_tlv(&data.apk.ver.partial_slice(5))?;
            writer.write_tlv(data.apk.sign)?;
            writer
        }
        0x154 => (data.sig.seq + 1).to_be_bytes().to_vec(),
        0x16e => data.device.model.as_bytes().to_vec(),
        0x174 => data.sig.t174.clone(),
        0x177 => {
            let mut writer = Vec::with_capacity(1 + 4 + 2 + data.apk.sdkver.len());
            writer.write_u8(0x01)?;
            writer.write_u32(data.apk.buildtime)?;
            writer.write_tlv(data.apk.sdkver)?;
            writer
        }
        0x17a => 9u32.to_be_bytes().to_vec(),
        0x17c => {
            let code = code.ok_or(TlvError::CodeNotProvided)?;
            let mut writer = Vec::with_capacity(2 + code.len());
            writer.write_tlv(code)?;
            writer
        }
        0x187 => md5::compute(&data.device.mac_address).0.to_vec(),
        0x188 => md5::compute(&data.device.android_id).0.to_vec(),
        0x191 => 0x82u8.to_be_bytes().to_vec(),
        0x193 => ticket
            .ok_or(TlvError::TicketNotProvided)?
            .as_bytes()
            .to_vec(),
        0x194 => data.device.imsi.to_vec(),
        0x197 | 0x198 => {
            let mut writer = Vec::with_capacity(2 + 1);
            writer.write_tlv(BUF_1)?;
            writer
        }
        0x202 => {
            let mut writer = Vec::with_capacity(2 + 16 + 2 + 32);
            writer.write_tlv(&data.device.wifi_bssid.partial_slice(16))?;
            writer.write_tlv(&data.device.wifi_ssid.partial_slice(32))?;
            writer
        }
        0x400 => {
            let mut writer = Vec::with_capacity(2 + 8 + 16 + 16 + 4 + 4 + 4 + 0);
            writer.write_u16(1)?;
            writer.write_u64(data.uin as u64)?;
            writer.write_bytes(data.device.guid)?;
            writer.write_bytes(rand::random::<[u8; 16]>())?;
            writer.write_i32(1)?;
            writer.write_i32(16)?;
            writer.write_u32(current_unix_timestamp_as_millis() as u32)?;
            writer.write_bytes(BUF_0)?;
            writer
        }
        0x401 => rand::random::<[u8; 16]>().to_vec(),
        0x511 => {
            let domains = [
                "aq.qq.com",
                "buluo.qq.com",
                "connect.qq.com",
                "docs.qq.com",
                "game.qq.com",
                "gamecenter.qq.com",
                "haoma.qq.com",
                "id.qq.com",
                "kg.qq.com",
                "mail.qq.com",
                "mma.qq.com",
                "office.qq.com",
                "openmobile.qq.com",
                "qqweb.qq.com",
                "qun.qq.com",
                "qzone.qq.com",
                "ti.qq.com",
                "v.qq.com",
                "vip.qq.com",
                "y.qq.com",
            ];

            let mut writer = Vec::with_capacity(2 + 20 * 3 + 189);
            writer.write_u16(domains.len() as u16)?;
            domains.iter().try_for_each(|domain| {
                writer.write_u8(0x01)?;
                writer.write_tlv(domain)
            })?;
            writer
        }
        0x516 => 0u32.to_be_bytes().to_vec(),
        0x521 => vec![0, 0, 0, 0, 0, 0],
        0x525 => {
            let mut writer = Vec::with_capacity(2 + 2 + 2 + 2);
            writer.write_u16(1)?;
            writer.write_u16(0x536)?;
            writer.write_tlv([0x1, 0x0])?;
            writer
        }
        0x52d => {
            let device = &data.device;
            let buf = encode(&ProtobufObject::from([
                (1, ProtobufElement::from(device.bootloader)),
                (2, ProtobufElement::from(device.proc_version.as_str())),
                (3, ProtobufElement::from(device.version.codename)),
                (4, ProtobufElement::from(device.version.incremental)),
                (5, ProtobufElement::from(device.fingerprint.as_str())),
                (6, ProtobufElement::from(device.boot_id.as_str())),
                (7, ProtobufElement::from(device.android_id.as_str())),
                (8, ProtobufElement::from(device.baseband)),
                (9, ProtobufElement::from(device.version.incremental)),
            ]))?;

            buf
        }
        _ => return Err(Error::from(TlvError::InvalidData)),
    };

    Ok(result)
}

pub fn pack(data: &DataCenter, tag: u16) -> Result<Vec<u8>, Error> {
    pack_with_args(data, tag, None, None, None, None)
}

pub fn pack_with_args(
    data: &DataCenter,
    tag: u16,
    emp: Option<u32>,
    md5_password: Option<[u8; 16]>,
    code: Option<Vec<u8>>,
    ticket: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let mut body = pack_body(data, tag, emp, md5_password, code, ticket)?;

    let a = (body.len() as u16).to_be_bytes();
    let b = tag.to_be_bytes();
    let append = [b[0], b[1], a[0], a[1]];
    body.splice(0..0, append);

    Ok(body)
}

pub trait WriteTlvExt: Write {
    fn write_tlv<B>(&mut self, buf: B) -> std::io::Result<()>
    where
        B: AsRef<[u8]>,
    {
        let buf = buf.as_ref();
        self.write_all(&(buf.len() as u16).to_be_bytes())?;
        self.write_all(buf)?;

        Ok(())
    }
}

impl<W: Write> WriteTlvExt for W {}

pub trait ReadTlvExt: Read {
    fn read_tlv(&mut self) -> Result<HashMap<u16, Vec<u8>>, std::io::Error> {
        let mut result = HashMap::new();
        let mut tag_buf = [0; 2];
        let mut len_buf = [0; 2];
        loop {
            match self.read_exact(&mut tag_buf) {
                Ok(_) => {
                    let tag = u16::from_be_bytes(tag_buf);
                    self.read_exact(&mut len_buf)?;
                    let len = u16::from_be_bytes(len_buf);

                    let mut buf = vec![0; len as usize];
                    self.read_exact(&mut buf)?;
                    result.insert(tag, buf);
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Ok(result)
    }
}

impl<R: Read> ReadTlvExt for R {}
