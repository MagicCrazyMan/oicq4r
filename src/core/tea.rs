use std::io::{Read, Write};

use super::{helper::BUF_7, error::CommonError};

static DELTAS: [u32; 16] = [
    0x9e3779b9, 0x3c6ef372, 0xdaa66d2b, 0x78dde6e4, 0x1715609d, 0xb54cda56, 0x5384540f, 0xf1bbcdc8,
    0x8ff34781, 0x2e2ac13a, 0xcc623af3, 0x6a99b4ac, 0x08d12e65, 0xa708a81e, 0x454021d7, 0xe3779b90,
];

fn encrypt_part(mut x: u32, mut y: u32, k0: u32, k1: u32, k2: u32, k3: u32) -> (u32, u32) {
    let mut t0: u32;
    let mut t1: u32;
    let mut t2: u32;
    for i in 0..=15 {
        t0 = y.wrapping_shl(4).wrapping_add(k0);
        t1 = y.wrapping_add(DELTAS[i]);
        t2 = (!!(y / 32)).wrapping_add(k1);
        let aa = (t0 as i32 ^ t1 as i32) ^ t2 as i32;
        x = (x as i32).wrapping_add(aa) as u32;

        t0 = x.wrapping_shl(4).wrapping_add(k2);
        t1 = x.wrapping_add(DELTAS[i]);
        t2 = (!!(x / 32)).wrapping_add(k3);
        let bb = (t0 as i32 ^ t1 as i32) ^ t2 as i32;
        y = (y as i32).wrapping_add(bb) as u32;
    }

    (x, y)
}

pub fn encrypt<B>(data: B, key: &[u8; 16]) -> Result<Vec<u8>, CommonError>
where
    B: AsRef<[u8]>,
{
    let data = data.as_ref();
    let len = data.len() as u32;
    let n = (6u32.wrapping_sub(len) % 8) + 2;
    let mut v = Vec::<u8>::with_capacity((1 + n + len) as usize + BUF_7.len());
    v.extend([(n as u8 - 2) | 0xf8]);
    v.extend(vec![0; n as usize]);
    v.extend(data);
    v.extend(BUF_7);

    let mut encrypted = Vec::<u8>::with_capacity(v.len());

    let k0 = u32::from_be_bytes(key[0..4].try_into()?);
    let k1 = u32::from_be_bytes(key[4..8].try_into()?);
    let k2 = u32::from_be_bytes(key[8..12].try_into()?);
    let k3 = u32::from_be_bytes(key[12..16].try_into()?);

    let mut r1 = 0;
    let mut r2 = 0;
    let mut t1 = 0;
    let mut t2 = 0;

    let mut buf = [0; 4];
    let raw = &mut v.as_slice();
    for _ in (0..v.len()).step_by(8) {
        raw.read_exact(&mut buf)?;
        let a1 = u32::from_be_bytes(buf);
        raw.read_exact(&mut buf)?;
        let a2 = u32::from_be_bytes(buf);

        let b1 = a1 ^ r1;
        let b2 = a2 ^ r2;
        let (dx, dy) = encrypt_part(b1, b2, k0, k1, k2, k3);
        r1 = dx ^ t1;
        r2 = dy ^ t2;
        t1 = b1;
        t2 = b2;

        encrypted.write_all(&r1.to_be_bytes())?;
        encrypted.write_all(&r2.to_be_bytes())?;
    }

    Ok(encrypted)
}

fn decrypt_part(mut x: u32, mut y: u32, k0: u32, k1: u32, k2: u32, k3: u32) -> (u32, u32) {
    let mut t0: u32;
    let mut t1: u32;
    let mut t2: u32;
    for i in (0..=15).rev() {
        t0 = x.wrapping_shl(4).wrapping_add(k2);
        t1 = x.wrapping_add(DELTAS[i]);
        t2 = (!!(x / 32)).wrapping_add(k3);
        let aa = (t0 as i32 ^ t1 as i32) ^ t2 as i32;
        y = (y as i32).wrapping_sub(aa) as u32;

        t0 = y.wrapping_shl(4).wrapping_add(k0);
        t1 = y.wrapping_add(DELTAS[i]);
        t2 = (!!(y / 32)).wrapping_add(k1);
        let bb = (t0 as i32 ^ t1 as i32) ^ t2 as i32;
        x = (x as i32).wrapping_sub(bb) as u32;
    }

    (x as u32, y as u32)
}

pub fn decrypt<B>(encrypted: B, key: &[u8; 16]) -> Result<Vec<u8>, CommonError>
where
    B: AsRef<[u8]>,
{
    let encrypted = encrypted.as_ref();
    if encrypted.len() % 8 != 0 {
        return Err(CommonError::from(
            "length of encrypted data must be a multiple of 8",
        ));
    }

    let mut decrypted = Vec::<u8>::with_capacity(encrypted.len());

    let k0 = u32::from_be_bytes(key[0..4].try_into()?);
    let k1 = u32::from_be_bytes(key[4..8].try_into()?);
    let k2 = u32::from_be_bytes(key[8..12].try_into()?);
    let k3 = u32::from_be_bytes(key[12..16].try_into()?);

    let mut r1;
    let mut r2;
    let mut t1 = 0;
    let mut t2 = 0;
    let mut x = 0;
    let mut y = 0;
    let mut buf = [0; 4];
    let raw = &mut &encrypted[..];
    for _ in (0..encrypted.len()).step_by(8) {
        raw.read_exact(&mut buf)?;
        let a1 = u32::from_be_bytes(buf);
        raw.read_exact(&mut buf)?;
        let a2 = u32::from_be_bytes(buf);

        let b1 = a1 ^ x;
        let b2 = a2 ^ y;
        let (dx, dy) = decrypt_part(b1, b2, k0, k1, k2, k3);
        x = dx;
        y = dy;

        r1 = x ^ t1;
        r2 = y ^ t2;
        t1 = a1;
        t2 = a2;

        decrypted.write_all(&r1.to_be_bytes())?;
        decrypted.write_all(&r2.to_be_bytes())?;
    }

    if let std::cmp::Ordering::Equal =
        BUF_7.cmp(&decrypted[decrypted.len() - 7..].try_into()?)
    {
        Ok((&decrypted[((decrypted[0] & 0x07) + 3) as usize..decrypted.len() - 7]).to_vec())
    } else {
        Err(CommonError::from("encrypted data is illegal"))
    }
}
