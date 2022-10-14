use std::{fs::File, io::Read};

use oicq4r::core::{
    helper::current_unix_timestamp_as_secs,
    tea,
    tlv::{ReadTlvExt, TlvError},
};

fn main() {
    let mut file = File::open("/home/magiccrazyman/Development/oicq/lib/sdfsdff").unwrap();
    let mut buf = Vec::with_capacity(2048);
    file.read_to_end(&mut buf).unwrap();

    println!("{}", buf.len());
    let decrypted = tea::decrypt(
        buf,
        &[
            0x2c, 0x54, 0x45, 0x73, 0x7e, 0x37, 0x76, 0x2a, 0x76, 0x60, 0x24, 0x58, 0x47, 0x5f,
            0x59, 0x46,
        ],
    )
    .unwrap();

    let mut t = (&mut &decrypted[2..]).read_tlv().unwrap();

    let tgt = t
        .remove(&0x10a)
        .ok_or(TlvError::TagNotExisted(0x10a))
        .unwrap();

    println!("{:x?}", tgt);
    let skey = t
        .remove(&0x120)
        .ok_or(TlvError::TagNotExisted(0x120))
        .unwrap();
    let d2 = t
        .remove(&0x143)
        .ok_or(TlvError::TagNotExisted(0x143))
        .unwrap();
    let d2key: [u8; 16] = t
        .remove(&0x305)
        .ok_or(TlvError::TagNotExisted(0x305))
        .unwrap()[..16]
        .try_into()
        .unwrap();
    let tgtgt = md5::compute(&d2key).0;
    let emp_time = current_unix_timestamp_as_secs();
}
