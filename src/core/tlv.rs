use std::{
    collections::HashMap,
    io::{Read, Write},
};

use super::{
    base_client::DataCenter,
    device::Platform,
    error::CommonError,
    helper::{current_unix_timestamp_as_millis, BUF_0, BUF_1, BUF_4},
    io::WriteExt,
    protobuf::{encode, ProtobufElement, ProtobufObject},
    tea::{self, encrypt},
};

trait PartialSlice {
    fn partial_slice(&self, max: usize) -> &str;
}

impl<T: AsRef<str>> PartialSlice for T {
    fn partial_slice(&self, max: usize) -> &str {
        let str = self.as_ref();
        let len = if str.len() > max { max } else { str.len() };
        &str[..len]
    }
}

fn pack_body<W: Write>(
    writer: &mut W,
    data: &DataCenter,
    tag: u16,
    emp: Option<u32>,
    md5_password: Option<[u8; 16]>,
    code: Option<Vec<u8>>,
    ticket: Option<Vec<u8>>,
) -> Result<(), CommonError> {
    match tag {
        0x01 => {
            writer.write_u16(1)?;
            writer.write_bytes(&rand::random::<[u8; 4]>())?;
            writer.write_u32(data.uin)?;
            writer.write_u32(current_unix_timestamp_as_millis() as u32)?;
            writer.write_bytes(BUF_4)?;
            writer.write_u16(0)?;
            Ok(())
        }
        0x08 => {
            writer.write_u16(0)?;
            writer.write_u32(2052)?;
            writer.write_u16(0)?;
            Ok(())
        }
        0x16 => {
            let apk = Platform::watch();
            writer.write_u32(7)?;
            writer.write_u32(apk.appid)?;
            writer.write_u32(apk.subid)?;
            writer.write_bytes(&data.device.guid)?;
            writer.write_tlv(apk.id)?;
            writer.write_tlv(apk.ver)?;
            writer.write_tlv(apk.sign)?;
            Ok(())
        }
        0x18 => {
            writer.write_u16(1)?;
            writer.write_u32(1536)?;
            writer.write_u32(data.apk.appid)?;
            writer.write_u32(0)?;
            writer.write_u32(data.uin)?;
            writer.write_u16(0)?;
            writer.write_u16(0)?;
            Ok(())
        }
        0x1B => {
            writer.write_u32(0)?;
            writer.write_u32(0)?;
            writer.write_u32(3)?;
            writer.write_u32(4)?;
            writer.write_u32(72)?;
            writer.write_u32(2)?;
            writer.write_u32(2)?;
            writer.write_u16(0)?;
            Ok(())
        }
        0x1D => {
            writer.write_u8(1)?;
            writer.write_u32(184024956)?;
            writer.write_u32(0)?;
            writer.write_u8(0)?;
            writer.write_u32(0)?;
            Ok(())
        }
        0x1F => {
            writer.write_u8(0)?;
            writer.write_tlv("android")?;
            writer.write_tlv("7.1.2")?;
            writer.write_u16(2)?;
            writer.write_tlv("China Mobile GSM")?;
            writer.write_tlv(BUF_0)?;
            writer.write_tlv("wifi")?;
            Ok(())
        }
        0x33 => {
            writer.write_bytes(data.device.guid)?;
            Ok(())
        }
        0x35 => {
            writer.write_u32(8)?;
            Ok(())
        }
        0x100 => {
            writer.write_u16(1)?;
            writer.write_u32(7)?;
            writer.write_u32(data.apk.appid)?;
            writer.write_u32(if emp.is_some() { 2 } else { data.apk.subid })?;
            writer.write_u32(0)?;
            writer.write_u32(data.apk.sigmap)?;
            Ok(())
        }
        0x104 => {
            writer.write_bytes(&data.sig.t104)?;
            Ok(())
        }
        0x106 => {
            let md5_password = md5_password.ok_or(CommonError::new("no password provided"))?;
            let mut body = Vec::with_capacity(100);
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
            body.write_tlv(data.uin.to_be_bytes())?;
            body.write_u16(0)?;

            let mut key = Vec::with_capacity(24);
            key.extend(md5_password);
            key.extend([0; 4]);
            key.extend(data.uin.to_be_bytes());
            let key = md5::compute(&key).0;

            body.extend(key);
            let encrypted = tea::encrypt(body, &key)?;
            writer.write_bytes(encrypted)?;
            Ok(())
        }
        0x107 => {
            writer.write_u16(0)?;
            writer.write_u8(0)?;
            writer.write_u16(0)?;
            writer.write_u8(1)?;
            Ok(())
        }
        0x109 => {
            writer.write_bytes(md5::compute(&data.device.imei).0)?;
            Ok(())
        }
        0x10a => {
            writer.write_bytes(&data.sig.tgt)?;
            Ok(())
        }
        0x116 => {
            writer.write_u8(0)?;
            writer.write_u32(data.apk.bitmap)?;
            writer.write_u32(0x10400)?;
            writer.write_u8(1)?;
            writer.write_u32(1600000226)?;
            Ok(())
        }
        0x124 => {
            writer.write_tlv(&data.device.os_type.partial_slice(16))?;
            writer.write_tlv(&data.device.version.release.partial_slice(16))?;
            writer.write_u16(2)?;
            writer.write_tlv(&data.device.sim.partial_slice(16))?;
            writer.write_u16(0)?;
            writer.write_tlv(&data.device.apn.partial_slice(16))?;
            Ok(())
        }
        0x128 => {
            writer.write_u16(0)?;
            writer.write_u8(0)?;
            writer.write_u8(1)?;
            writer.write_u8(0)?;
            writer.write_u32(16777216)?;
            writer.write_tlv(&data.device.model.partial_slice(32))?;
            writer.write_tlv(&data.device.guid)?;
            writer.write_tlv(&data.device.brand.partial_slice(16))?;
            Ok(())
        }
        0x141 => {
            writer.write_u16(1)?;
            writer.write_tlv(data.device.sim)?;
            writer.write_u16(2)?;
            writer.write_tlv(data.device.apn)?;
            Ok(())
        }
        0x142 => {
            writer.write_u16(0)?;
            writer.write_tlv(&data.apk.id.partial_slice(32))?;
            Ok(())
        }
        0x143 => {
            writer.write_bytes(&data.sig.d2)?;
            Ok(())
        }
        0x144 => {
            let mut body = Vec::with_capacity(200);
            body.write_u16(5)?;
            body.write_bytes(pack(data, 0x109)?)?;
            body.write_bytes(pack(data, 0x52d)?)?;
            body.write_bytes(pack(data, 0x124)?)?;
            body.write_bytes(pack(data, 0x128)?)?;
            body.write_bytes(pack(data, 0x16e)?)?;

            writer.write_bytes(encrypt(body, &data.sig.tgtgt)?)?;
            Ok(())
        }
        0x145 => {
            writer.write_bytes(data.device.guid)?;
            Ok(())
        }
        0x147 => {
            writer.write_u32(data.apk.appid)?;
            writer.write_tlv(&data.apk.ver.partial_slice(5))?;
            writer.write_tlv(data.apk.sign)?;
            Ok(())
        }
        0x154 => {
            writer.write_u32(data.sig.seq + 1)?;
            Ok(())
        }
        0x16e => {
            writer.write_bytes(data.device.model)?;
            Ok(())
        }
        0x174 => {
            writer.write_bytes(&data.sig.t174)?;
            Ok(())
        }
        0x177 => {
            writer.write_u8(0x01)?;
            writer.write_u32(data.apk.buildtime)?;
            writer.write_tlv(data.apk.sdkver)?;
            Ok(())
        }
        0x17a => {
            writer.write_u32(9)?;
            Ok(())
        }
        0x17c => {
            writer.write_tlv(code.unwrap())?;
            Ok(())
        }
        0x187 => {
            writer.write_bytes(md5::compute(&data.device.mac_address).0)?;
            Ok(())
        }
        0x188 => {
            writer.write_bytes(md5::compute(&data.device.android_id).0)?;
            Ok(())
        }
        0x191 => {
            writer.write_u8(0x82)?;
            Ok(())
        }
        0x193 => {
            writer.write_bytes(ticket.unwrap())?;
            Ok(())
        }
        0x194 => {
            writer.write_bytes(data.device.imsi)?;
            Ok(())
        }
        0x197 => {
            writer.write_tlv(BUF_1)?;
            Ok(())
        }
        0x198 => {
            writer.write_tlv(BUF_1)?;
            Ok(())
        }
        0x202 => {
            writer.write_tlv(&data.device.wifi_bssid.partial_slice(16))?;
            writer.write_tlv(&data.device.wifi_ssid.partial_slice(32))?;
            Ok(())
        }
        0x400 => {
            writer.write_u16(1)?;
            writer.write_u64(data.uin as u64)?;
            writer.write_bytes(data.device.guid)?;
            writer.write_bytes(rand::random::<[u8; 16]>())?;
            writer.write_i32(1)?;
            writer.write_i32(16)?;
            writer.write_u32(current_unix_timestamp_as_millis() as u32)?;
            writer.write_bytes(BUF_0)?;
            Ok(())
        }
        0x401 => {
            writer.write_bytes(rand::random::<[u8; 16]>())?;
            Ok(())
        }
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
            writer.write_u16(domains.len() as u16)?;
            domains.iter().try_for_each(|domain| {
                writer.write_u8(0x01)?;
                writer.write_tlv(domain)
            })?;
            Ok(())
        }
        0x516 => {
            writer.write_u32(0)?;
            Ok(())
        }
        0x521 => {
            writer.write_u32(0)?;
            writer.write_u16(0)?;
            Ok(())
        }
        0x525 => {
            writer.write_u16(1)?;
            writer.write_u16(0x536)?;
            writer.write_tlv([0x1, 0x0])?;
            Ok(())
        }
        0x52d => {
            let device = &data.device;
            let buf = encode(&ProtobufObject::from([
                (1, ProtobufElement::from(device.bootloader)),
                (2, ProtobufElement::from(device.proc_version.as_str())),
                (3, ProtobufElement::from(device.version.codename)),
                (4, ProtobufElement::from(device.version.incremental as i64)),
                (5, ProtobufElement::from(device.fingerprint.as_str())),
                (6, ProtobufElement::from(device.boot_id.as_str())),
                (7, ProtobufElement::from(device.android_id.as_str())),
                (8, ProtobufElement::from(device.baseband)),
                (9, ProtobufElement::from(device.version.incremental as i64)),
            ]))?;

            writer.write_bytes(&buf)?;
            Ok(())
        }
        _ => Err(CommonError::from("Invalid Input")),
    }
}

pub fn pack(data: &DataCenter, tag: u16) -> Result<Vec<u8>, CommonError> {
    pack_with_args(data, tag, None, None, None, None)
}

pub fn pack_with_args(
    data: &DataCenter,
    tag: u16,
    emp: Option<u32>,
    md5_password: Option<[u8; 16]>,
    code: Option<Vec<u8>>,
    ticket: Option<Vec<u8>>,
) -> Result<Vec<u8>, CommonError> {
    let mut body = Vec::with_capacity(512);
    pack_body(&mut body, data, tag, emp, md5_password, code, ticket)?;

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
