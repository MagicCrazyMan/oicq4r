use std::io::{Read, Result, Write};

macro_rules! write_number {
    ($(($n: ident, $t: ty)),+) => {
        $(
            fn $n(&mut self, n: $t) -> Result<()> {
                self.write_all(&n.to_be_bytes())
            }
        )+
    };
}

macro_rules! write_number_le {
    ($(($n: ident, $t: ty)),+) => {
        $(
            fn $n(&mut self, n: $t) -> Result<()> {
                self.write_all(&n.to_le_bytes())
            }
        )+
    };
}

pub trait WriteExt: Write {
    write_number! {
        (write_u8, u8),
        (write_i8, i8),
        (write_u16, u16),
        (write_i16, i16),
        (write_u32, u32),
        (write_i32, i32),
        (write_u64, u64),
        (write_i64, i64),
        (write_u128, u128),
        (write_i128, i128),
        (write_usize, usize),
        (write_isize, isize),
        (write_f32, f32),
        (write_f64, f64)
    }

    write_number_le! {
        (write_u16_le, u16),
        (write_i16_le, i16),
        (write_u32_le, u32),
        (write_i32_le, i32),
        (write_u64_le, u64),
        (write_i64_le, i64),
        (write_u128_le, u128),
        (write_i128_le, i128),
        (write_usize_le, usize),
        (write_isize_le, isize),
        (write_f32_le, f32),
        (write_f64_le, f64)
    }

    fn write_bytes<B>(&mut self, bytes: B) -> Result<()>
    where
        B: AsRef<[u8]>,
    {
        self.write_all(bytes.as_ref())
    }

    fn write_bytes_with_length<B>(&mut self, bytes: B) -> Result<()>
    where
        B: AsRef<[u8]>,
    {
        let bytes = bytes.as_ref();
        let len = (bytes.len() + 4) as u32;
        self.write_u32(len)?;
        self.write_all(bytes)
    }
}

impl<W: Write> WriteExt for W {}

macro_rules! read_number {
    ($(($n: ident, $t: ty)),+) => {
        $(
            fn $n(&mut self) -> Result<$t> {
                let mut buf = [0; <$t>::BITS as usize / 8];
                self.read_exact(&mut buf)?;
                Ok(<$t>::from_be_bytes(buf))
            }
        )+
    };
}

macro_rules! read_number_le {
    ($(($n: ident, $t: ty)),+) => {
        $(
            fn $n(&mut self) -> Result<$t> {
                let mut buf = [0; <$t>::BITS as usize / 8];
                self.read_exact(&mut buf)?;
                Ok(<$t>::from_le_bytes(buf))
            }
        )+
    };
}

pub trait ReadExt: Read {
    read_number! {
        (read_u8, u8),
        (read_i8, i8),
        (read_u16, u16),
        (read_i16, i16),
        (read_u32, u32),
        (read_i32, i32),
        (read_u64, u64),
        (read_i64, i64),
        (read_u128, u128),
        (read_i128, i128),
        (read_usize, usize),
        (read_isize, isize)
    }

    read_number_le! {
        (read_u16_le, u16),
        (read_i16_le, i16),
        (read_u32_le, u32),
        (read_i32_le, i32),
        (read_u64_le, u64),
        (read_i64_le, i64),
        (read_u128_le, u128),
        (read_i128_le, i128),
        (read_usize_le, usize),
        (read_isize_le, isize)
    }

    fn read_f32(&mut self) -> Result<f32> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(f32::from_be_bytes(buf))
    }

    fn read_f32_le(&mut self) -> Result<f32> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }

    fn read_f64(&mut self) -> Result<f64> {
        let mut buf = [0; 8];
        self.read_exact(&mut buf)?;
        Ok(f64::from_be_bytes(buf))
    }

    fn read_f64_le(&mut self) -> Result<f64> {
        let mut buf = [0; 8];
        self.read_exact(&mut buf)?;
        Ok(f64::from_le_bytes(buf))
    }

    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0; len];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl<R: Read> ReadExt for R {}
