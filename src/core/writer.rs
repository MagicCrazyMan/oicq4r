use std::io::Write;

#[inline]
pub fn write_u8<W>(writer: &mut W, number: u8) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_u16<W>(writer: &mut W, number: u16) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_u32<W>(writer: &mut W, number: u32) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_u64<W>(writer: &mut W, number: u64) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_i8<W>(writer: &mut W, number: i8) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_i16<W>(writer: &mut W, number: i16) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_i32<W>(writer: &mut W, number: i32) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_i64<W>(writer: &mut W, number: i64) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_f32<W>(writer: &mut W, number: f32) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_f64<W>(writer: &mut W, number: f64) -> Result<(), std::io::Error>
where
    W: Write,
{
    writer.write_all(&number.to_be_bytes())
}

#[inline]
pub fn write_bytes<W, B>(writer: &mut W, bytes: B) -> Result<(), std::io::Error>
where
    W: Write,
    B: AsRef<[u8]>,
{
    writer.write_all(bytes.as_ref())
}

pub fn write_bytes_with_length<W, B>(writer: &mut W, bytes: B) -> Result<(), std::io::Error>
where
    W: Write,
    B: AsRef<[u8]>,
{
    let bytes = bytes.as_ref();
    let len = (bytes.len() + 4) as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(bytes)
}

pub fn write_tlv<W, B>(writer: &mut W, bytes: B) -> Result<(), std::io::Error>
where
    W: Write,
    B: AsRef<[u8]>,
{
    let bytes = bytes.as_ref();
    let len = bytes.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(bytes)
}
