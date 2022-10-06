use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    base_client::BaseClientData,
    constant::{BUF_0, BUF_1},
    device::Platform,
    error::CommonError,
    protobuf::{encode, ProtobufElement, ProtobufObject},
    tea::{self, encrypt},
    io::WriteExt,
};

fn current_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

 fn pack_body<W: Write>(
    writer: &mut W,
    base_client: &BaseClientData,
    tag: u16,
    emp: Option<u32>,
    md5pass: Option<Vec<u8>>,
    code: Option<Vec<u8>>,
    ticket: Option<Vec<u8>>,
) -> Result<(), CommonError> {
    match tag {
        0x01 => {
            writer.write_u16(1)?;
            writer.write_bytes(&rand::random::<[u8; 4]>())?;
            writer.write_u32(base_client.uin())?;
            writer.write_bytes(&current_timestamp().to_be_bytes()[..32])?;
            writer.write_bytes(&[0; 4])?;
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
            writer.write_bytes(&base_client.device().guid)?;
            writer.write_tlv(apk.id)?;
            writer.write_tlv(apk.ver)?;
            writer.write_tlv(apk.sign)?;
            Ok(())
        }
        0x18 => {
            writer.write_u16(1)?;
            writer.write_u32(1536)?;
            writer.write_u32(base_client.apk().appid)?;
            writer.write_u32(0)?;
            writer.write_u32(base_client.uin())?;
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
            writer.write_u16(2)?;
            writer.write_u16(2)?;
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
            writer.write_u8(1)?;
            writer.write_tlv("android")?;
            writer.write_tlv("7.1.2")?;
            writer.write_u16(2)?;
            writer.write_tlv("China Mobile GSM")?;
            writer.write_tlv([])?;
            writer.write_tlv("wifi")?;
            Ok(())
        }
        0x33 => {
            writer.write_bytes(base_client.device().guid)?;
            Ok(())
        }
        0x35 => {
            writer.write_u32(8)?;
            Ok(())
        }
        0x100 => {
            writer.write_u16(1)?;
            writer.write_u32(7)?;
            writer.write_u32(base_client.apk().appid)?;
            writer.write_u32(if emp.is_some() {
                2
            } else {
                base_client.apk().subid
            })?;
            writer.write_u32(8)?;
            writer.write_u32(8)?;
            Ok(())
        }
        0x104 => {
            writer.write_bytes(base_client.sig().t104)?;
            Ok(())
        }
        0x106 => {
            let md5pass = md5pass.unwrap();
            let mut body = Vec::with_capacity(100);
            body.write_u16(4)?;
            body.write_bytes(rand::random::<[u8; 4]>())?;
            body.write_u32(7)?;
            body.write_u32(base_client.apk().appid)?;
            body.write_u32(0)?;
            body.write_u64(base_client.uin() as u64)?;
            body.write_bytes(&current_timestamp().to_be_bytes()[..32])?;
            body.write_bytes([0; 4])?;
            body.write_u8(1)?;
            body.write_bytes(&md5pass)?;
            body.write_bytes(base_client.sig().tgtgt)?;
            body.write_u32(0)?;
            body.write_u8(1)?;
            body.write_bytes(base_client.device().guid)?;
            body.write_u32(base_client.apk().subid)?;
            body.write_u32(1)?;
            body.write_tlv(base_client.uin().to_be_bytes())?;
            body.write_u16(0)?;

            let mut key = md5pass.clone();
            key.extend([0; 4]);
            key.extend(base_client.uin().to_be_bytes());
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
            writer.write_bytes(md5::compute(&base_client.device().imei).0)?;
            Ok(())
        }
        0x10a => {
            writer.write_bytes(base_client.sig().tgt)?;
            Ok(())
        }
        0x116 => {
            writer.write_u8(0)?;
            writer.write_u32(base_client.apk().bitmap)?;
            writer.write_u32(0x10400)?;
            writer.write_u8(1)?;
            writer.write_u32(1600000226)?;
            Ok(())
        }
        0x124 => {
            writer.write_tlv(&base_client.device().os_type[..16])?;
            writer.write_tlv(&base_client.device().version.release[..16])?;
            writer.write_u16(2)?;
            writer.write_tlv(&base_client.device().sim[..16])?;
            writer.write_u16(0)?;
            writer.write_tlv(&base_client.device().apn[..16])?;
            Ok(())
        }
        0x128 => {
            writer.write_u16(0)?;
            writer.write_u8(0)?;
            writer.write_u8(1)?;
            writer.write_u8(0)?;
            writer.write_u32(16777216)?;
            writer.write_tlv(&base_client.device().model[..32])?;
            writer.write_tlv(&base_client.device().guid[..16])?;
            writer.write_tlv(&base_client.device().brand[..16])?;
            Ok(())
        }
        0x141 => {
            writer.write_u16(1)?;
            writer.write_tlv(base_client.device().sim)?;
            writer.write_u16(2)?;
            writer.write_tlv(base_client.device().apn)?;
            Ok(())
        }
        0x142 => {
            writer.write_u16(0)?;
            writer.write_tlv(&base_client.apk().id[..32])?;
            Ok(())
        }
        0x143 => {
            writer.write_bytes(base_client.sig().d2)?;
            Ok(())
        }
        0x144 => {
            let mut body = Vec::with_capacity(200);
            body.write_u16(5)?;
            body.write_bytes(pack_tlv(base_client, 0x109)?)?;
            body.write_bytes(pack_tlv(base_client, 0x52d)?)?;
            body.write_bytes(pack_tlv(base_client, 0x124)?)?;
            body.write_bytes(pack_tlv(base_client, 0x128)?)?;
            body.write_bytes(pack_tlv(base_client, 0x16e)?)?;

            writer.write_bytes(encrypt(body, &base_client.sig().tgtgt)?)?;
            Ok(())
        }
        0x145 => {
            writer.write_bytes(base_client.device().guid)?;
            Ok(())
        }
        0x147 => {
            writer.write_u32(base_client.apk().appid)?;
            writer.write_tlv(&base_client.apk().ver[..5])?;
            writer.write_tlv(base_client.apk().sign)?;
            Ok(())
        }
        0x154 => {
            writer.write_u32(base_client.sig().seq + 1)?;
            Ok(())
        }
        0x16e => {
            writer.write_bytes(base_client.device().model)?;
            Ok(())
        }
        0x174 => {
            writer.write_bytes(base_client.sig().t174)?;
            Ok(())
        }
        0x177 => {
            writer.write_u8(0x01)?;
            writer.write_u32(base_client.apk().buildtime)?;
            writer.write_tlv(base_client.apk().sdkver)?;
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
            writer.write_bytes(md5::compute(&base_client.device().mac_address).0)?;
            Ok(())
        }
        0x188 => {
            writer.write_bytes(md5::compute(&base_client.device().android_id).0)?;
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
            writer.write_bytes(base_client.device().imsi)?;
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
            writer.write_tlv(&base_client.device().wifi_bssid[..16])?;
            writer.write_tlv(&base_client.device().wifi_ssid[..32])?;
            Ok(())
        }
        0x400 => {
            writer.write_u16(1)?;
            writer.write_u64(base_client.uin() as u64)?;
            writer.write_bytes(base_client.device().guid)?;
            writer.write_bytes(rand::random::<[u8; 16]>())?;
            writer.write_i32(1)?;
            writer.write_i32(16)?;
            writer.write_bytes(&current_timestamp().to_be_bytes()[..32])?;
            writer.write_bytes(BUF_0)?;
            Ok(())
        }
        0x401 => {
            writer.write_bytes(rand::random::<[u8; 16]>())?;
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

            writer.write_bytes(&buf)?;
            Ok(())
        }
        _ => Err(CommonError::from("Invalid Input")),
    }
}

pub  fn pack_tlv(base_client: &BaseClientData, tag: u16) -> Result<Vec<u8>, CommonError> {
    let mut body = Vec::with_capacity(200);
    pack_body(&mut body, base_client, tag, None, None, None, None)?;
    let len = body.len();
    body.write_u16(len as u16)?;
    body.write_u16(tag)?;

    Ok(body)
}

pub trait WriteTlvExt: Write {
    fn write_tlv<B>(&mut self, buf: B) -> std::io::Result<()>
    where
        B: AsRef<[u8]>,
    {
        let buf = buf.as_ref();
        self.write_all(&(buf.len() as u32).to_be_bytes())?;
        self.write_all(buf)?;

        Ok(())
    }
}

impl<W: Write> WriteTlvExt for W {}

// pin_project! {
//     #[derive(Debug)]
//     #[must_use = "futures do nothing unless you `.await` or poll them"]
//     pub struct WriteTlv<'a, W: ?Sized> {
//         writer:&'a mut W,
//         buf: &'a [u8],
//         #[pin]
//         _pin: PhantomPinned,
//     }
// }

// impl<W> Future for WriteTlv<'_, W>
// where
//     W: ?Sized + Unpin + AsyncWrite,
// {
//     type Output = std::io::Result<()>;

//     fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
//         let me = self.project();

//         let len_bytes = (me.buf.len() as u32).to_be_bytes();
//         let buf = &mut (&len_bytes as &[u8]);
//         while !buf.is_empty() {
//             let n = ready!(Pin::new(&mut *me.writer).poll_write(cx, buf))?;
//             {
//                 let (_, rest) = std::mem::take(&mut *buf).split_at(n);
//                 *buf = rest;
//             }
//             if n == 0 {
//                 return Poll::Ready(Err(std::io::ErrorKind::WriteZero.into()));
//             }
//         }

//         while !me.buf.is_empty() {
//             let n = ready!(Pin::new(&mut *me.writer).poll_write(cx, me.buf))?;
//             {
//                 let (_, rest) = std::mem::take(&mut *me.buf).split_at(n);
//                 *me.buf = rest;
//             }
//             if n == 0 {
//                 return Poll::Ready(Err(std::io::ErrorKind::WriteZero.into()));
//             }
//         }

//         Poll::Ready(Ok(()))
//     }
// }

// pub trait AsyncWriteTlvExt: AsyncWrite {
//     fn write_tlv<'a>(&'a mut self, buf: &'a [u8]) -> WriteTlv<'a, Self> {
//         WriteTlv {
//             writer: self,
//             buf,
//             _pin: PhantomPinned,
//         }
//     }
// }

// impl<W: AsyncWrite + ?Sized> AsyncWriteTlvExt for W {}
