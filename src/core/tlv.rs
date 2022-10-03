use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    base_client::BaseClient,
    constant::{BUF_1, BUF_0},
    device::Platform,
    error::CommonError,
    protobuf::{encode, ProtobufElement, ProtobufObject},
    tea,
    writer::{write_bytes, write_i32, write_tlv, write_u16, write_u32, write_u64, write_u8},
};

fn current_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

async fn pack_body<W: Write>(
    writer: &mut W,
    base_client: &BaseClient,
    tag: u16,
    emp: Option<u32>,
    md5pass: Option<Vec<u8>>,
    code: Option<Vec<u8>>,
    ticket: Option<Vec<u8>>,
) -> Result<(), CommonError> {
    match tag {
        0x01 => {
            write_u16(writer, 1)?;
            write_bytes(writer, &rand::random::<[u8; 4]>())?;
            write_u32(writer, base_client.uin())?;
            write_bytes(writer, &current_timestamp().to_be_bytes()[..32])?;
            write_bytes(writer, &[0; 4])?;
            write_u16(writer, 0)?;
            Ok(())
        }
        0x08 => {
            write_u16(writer, 0)?;
            write_u32(writer, 2052)?;
            write_u16(writer, 0)?;
            Ok(())
        }
        0x16 => {
            let apk = Platform::watch();
            write_u32(writer, 7)?;
            write_u32(writer, apk.appid)?;
            write_u32(writer, apk.subid)?;
            write_bytes(writer, &base_client.device().guid)?;
            write_tlv(writer, apk.id)?;
            write_tlv(writer, apk.ver)?;
            write_tlv(writer, apk.sign)?;
            Ok(())
        }
        0x18 => {
            write_u16(writer, 1)?;
            write_u32(writer, 1536)?;
            write_u32(writer, base_client.apk().appid)?;
            write_u32(writer, 0)?;
            write_u32(writer, base_client.uin())?;
            write_u16(writer, 0)?;
            write_u16(writer, 0)?;
            Ok(())
        }
        0x1B => {
            write_u32(writer, 0)?;
            write_u32(writer, 0)?;
            write_u32(writer, 3)?;
            write_u32(writer, 4)?;
            write_u32(writer, 72)?;
            write_u16(writer, 2)?;
            write_u16(writer, 2)?;
            write_u16(writer, 0)?;
            Ok(())
        }
        0x1D => {
            write_u8(writer, 1)?;
            write_u32(writer, 184024956)?;
            write_u32(writer, 0)?;
            write_u8(writer, 0)?;
            write_u32(writer, 0)?;
            Ok(())
        }
        0x1F => {
            write_u8(writer, 1)?;
            write_tlv(writer, "android")?;
            write_tlv(writer, "7.1.2")?;
            write_u16(writer, 2)?;
            write_tlv(writer, "China Mobile GSM")?;
            write_tlv(writer, [])?;
            write_tlv(writer, "wifi")?;
            Ok(())
        }
        0x33 => {
            write_bytes(writer, base_client.device().guid)?;
            Ok(())
        }
        0x35 => {
            write_u32(writer, 8)?;
            Ok(())
        }
        0x100 => {
            write_u16(writer, 1)?;
            write_u32(writer, 7)?;
            write_u32(writer, base_client.apk().appid)?;
            write_u32(
                writer,
                if emp.is_some() {
                    2
                } else {
                    base_client.apk().subid
                },
            )?;
            write_u32(writer, 8)?;
            write_u32(writer, 8)?;
            Ok(())
        }
        0x104 => {
            write_bytes(writer, base_client.sig().await.t104)?;
            Ok(())
        }
        0x106 => {
            let md5pass = md5pass.unwrap();
            let mut body = Vec::with_capacity(100);
            write_u16(&mut body, 4)?;
            write_bytes(&mut body, rand::random::<[u8; 4]>())?;
            write_u32(&mut body, 7)?;
            write_u32(&mut body, base_client.apk().appid)?;
            write_u32(&mut body, 0)?;
            write_u64(&mut body, base_client.uin() as u64)?;
            write_bytes(&mut body, &current_timestamp().to_be_bytes()[..32])?;
            write_bytes(&mut body, [0; 4])?;
            write_u8(&mut body, 1)?;
            write_bytes(&mut body, &md5pass)?;
            write_bytes(&mut body, base_client.sig().await.tgtgt)?;
            write_u32(&mut body, 0)?;
            write_u8(&mut body, 1)?;
            write_bytes(&mut body, base_client.device().guid)?;
            write_u32(&mut body, base_client.apk().subid)?;
            write_u32(&mut body, 1)?;
            write_tlv(&mut body, base_client.uin().to_string())?;
            write_u16(&mut body, 0)?;

            let mut key = md5pass.clone();
            key.extend([0; 4]);
            key.extend(base_client.uin().to_be_bytes());
            let key = md5::compute(&key).0;

            body.extend(key);
            let encrypted = tea::encrypt(body, &key)?;
            write_bytes(writer, encrypted)?;
            Ok(())
        }
        0x107 => {
            write_u16(writer, 0)?;
            write_u8(writer, 0)?;
            write_u16(writer, 0)?;
            write_u8(writer, 1)?;
            Ok(())
        }
        0x109 => {
            write_bytes(writer, md5::compute(&base_client.device().imei).0)?;
            Ok(())
        }
        0x10a => {
            write_bytes(writer, base_client.sig().await.tgt)?;
            Ok(())
        }
        0x116 => {
            write_u8(writer, 0)?;
            write_u32(writer, base_client.apk().bitmap)?;
            write_u32(writer, 0x10400)?;
            write_u8(writer, 1)?;
            write_u32(writer, 1600000226)?;
            Ok(())
        }
        0x124 => {
            write_tlv(writer, &base_client.device().os_type[..16])?;
            write_tlv(writer, &base_client.device().version.release[..16])?;
            write_u16(writer, 2)?;
            write_tlv(writer, &base_client.device().sim[..16])?;
            write_u16(writer, 0)?;
            write_tlv(writer, &base_client.device().apn[..16])?;
            Ok(())
        }
        0x128 => {
            write_u16(writer, 0)?;
            write_u8(writer, 0)?;
            write_u8(writer, 1)?;
            write_u8(writer, 0)?;
            write_u32(writer, 16777216)?;
            write_tlv(writer, &base_client.device().model[..32])?;
            write_tlv(writer, &base_client.device().guid[..16])?;
            write_tlv(writer, &base_client.device().brand[..16])?;
            Ok(())
        }
        0x141 => {
            write_u16(writer, 1)?;
            write_tlv(writer, base_client.device().sim)?;
            write_u16(writer, 2)?;
            write_tlv(writer, base_client.device().apn)?;
            Ok(())
        }
        0x142 => {
            write_u16(writer, 0)?;
            write_tlv(writer, &base_client.apk().id[..32])?;
            Ok(())
        }
        0x143 => {
            write_bytes(writer, base_client.sig().await.d2)?;
            Ok(())
        }
        // 0x144 => {
        //     write_u16(writer, 1)?;
        //     write_bytes(writer, ma)?;
        //     write_bytes(writer, 2)?;
        //     write_bytes(writer, base_client.device().apn)?;
        //     write_bytes(writer, base_client.device().apn)?;
        //     write_bytes(writer, base_client.device().apn)?;
        //     Ok(())
        // }
        0x145 => {
            write_bytes(writer, base_client.device().guid)?;
            Ok(())
        }
        0x147 => {
            write_u32(writer, base_client.apk().appid)?;
            write_tlv(writer, &base_client.apk().ver[..5])?;
            write_tlv(writer, base_client.apk().sign)?;
            Ok(())
        }
        0x154 => {
            write_u32(writer, base_client.sig().await.seq + 1)?;
            Ok(())
        }
        0x16e => {
            write_bytes(writer, base_client.device().model)?;
            Ok(())
        }
        0x174 => {
            write_bytes(writer, base_client.sig().await.t174)?;
            Ok(())
        }
        0x177 => {
            write_u8(writer, 0x01)?;
            write_u32(writer, base_client.apk().buildtime)?;
            write_tlv(writer, base_client.apk().sdkver)?;
            Ok(())
        }
        0x17a => {
            write_u32(writer, 9)?;
            Ok(())
        }
        0x17c => {
            write_tlv(writer, code.unwrap())?;
            Ok(())
        }
        0x187 => {
            write_bytes(writer, md5::compute(&base_client.device().mac_address).0)?;
            Ok(())
        }
        0x188 => {
            write_bytes(writer, md5::compute(&base_client.device().android_id).0)?;
            Ok(())
        }
        0x191 => {
            write_u8(writer, 0x82)?;
            Ok(())
        }
        0x193 => {
            write_bytes(writer, ticket.unwrap())?;
            Ok(())
        }
        0x194 => {
            write_bytes(writer, base_client.device().imsi)?;
            Ok(())
        }
        0x197 => {
            write_tlv(writer, BUF_1)?;
            Ok(())
        }
        0x198 => {
            write_tlv(writer, BUF_1)?;
            Ok(())
        }
        0x202 => {
            write_tlv(writer, &base_client.device().wifi_bssid[..16])?;
            write_tlv(writer, &base_client.device().wifi_ssid[..32])?;
            Ok(())
        }
        0x400 => {
            write_u16(writer, 1)?;
            write_u64(writer, base_client.uin() as u64)?;
            write_bytes(writer, base_client.device().guid)?;
            write_bytes(writer, rand::random::<[u8; 16]>())?;
            write_i32(writer, 1)?;
            write_i32(writer, 16)?;
            write_bytes(writer, &current_timestamp().to_be_bytes()[..32])?;
            write_bytes(writer, BUF_0)?;
            Ok(())
        }
        0x401 => {
            write_bytes(writer, rand::random::<[u8; 16]>())?;
            Ok(())
        }
        0x511 => {
            let domains = [
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
            write_u16(writer, domains.len() as u16)?;
            domains.iter().try_for_each(|domain| {
                write_u8(writer, 0x01)?;
                write_tlv(writer, domain)
            })?;
            Ok(())
        }
        0x516 => {
            write_u32(writer, 0)?;
            Ok(())
        }
        0x521 => {
            write_u32(writer, 0)?;
            write_u16(writer, 0)?;
            Ok(())
        }
        0x525 => {
            write_u16(writer, 1)?;
            write_u16(writer, 0x536)?;
            write_tlv(writer, [0x1, 0x0])?;
            Ok(())
        }
        0x52d => {
            let device = base_client.device();
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

            write_bytes(writer, &buf)?;
            Ok(())
        }
        _ => Err(CommonError::from("Invalid Input")),
    }
}

pub async fn pack_tlv(base_client: &BaseClient, tag: u16) -> Result<Vec<u8>, CommonError> {
    let mut body = Vec::with_capacity(200);
    pack_body(&mut body, base_client, tag, None, None, None, None).await?;
    let len = body.len();
    write_u16(&mut body, len as u16)?;
    write_u16(&mut body, tag)?;

    Ok(body)
}
