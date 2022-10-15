use std::{
    collections::HashMap,
    fmt::Display,
    io::{Read, Write},
};

use protobuf::{CodedInputStream, CodedOutputStream};

#[derive(Debug)]
pub enum ProtobufError {
    InvalidType(u32),
    ItemNotFound(u32),
    NotInteger,
    NotDouble,
    NotBytes,
    NotString,
    NotArray,
    NotObject,
    RawError(protobuf::Error),
}

impl std::error::Error for ProtobufError {}

impl Display for ProtobufError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtobufError::InvalidType(typee) => {
                f.write_fmt(format_args!("invalid type: {}.", typee))
            }
            ProtobufError::ItemNotFound(e) => f.write_fmt(format_args!("item not found: {}.", *e)),
            ProtobufError::RawError(e) => e.fmt(f),
            ProtobufError::NotInteger => f.write_str("type of this protobuf element not Integer"),
            ProtobufError::NotDouble => f.write_str("type of this protobuf element not Double"),
            ProtobufError::NotBytes => f.write_str("type of this protobuf element not Bytes"),
            ProtobufError::NotString => f.write_str("type of this protobuf element not String"),
            ProtobufError::NotArray => f.write_str("type of this protobuf element not Array"),
            ProtobufError::NotObject => {
                f.write_str("type of this protobuf element not ProtobufObject")
            }
        }
    }
}

impl From<protobuf::Error> for ProtobufError {
    fn from(err: protobuf::Error) -> Self {
        Self::RawError(err)
    }
}

#[derive(Debug, Clone)]
pub enum ProtobufElement {
    Null,
    Integer(isize),
    Double(f64),
    Bytes(Vec<u8>),
    String(String),
    Array(Vec<ProtobufElement>),
    Object(ProtobufObject),
}

impl TryFrom<ProtobufElement> for isize {
    type Error = ProtobufError;

    fn try_from(p: ProtobufElement) -> Result<Self, Self::Error> {
        if let ProtobufElement::Integer(v) = p {
            Ok(v)
        } else {
            Err(ProtobufError::NotInteger)
        }
    }
}

impl TryFrom<ProtobufElement> for f64 {
    type Error = ProtobufError;

    fn try_from(p: ProtobufElement) -> Result<Self, Self::Error> {
        if let ProtobufElement::Double(v) = p {
            Ok(v)
        } else {
            Err(ProtobufError::NotDouble)
        }
    }
}

impl TryFrom<ProtobufElement> for Vec<u8> {
    type Error = ProtobufError;

    fn try_from(p: ProtobufElement) -> Result<Self, Self::Error> {
        if let ProtobufElement::Bytes(v) = p {
            Ok(v)
        } else {
            Err(ProtobufError::NotBytes)
        }
    }
}

impl TryFrom<ProtobufElement> for String {
    type Error = ProtobufError;

    fn try_from(p: ProtobufElement) -> Result<Self, Self::Error> {
        if let ProtobufElement::String(v) = p {
            Ok(v)
        } else {
            Err(ProtobufError::NotString)
        }
    }
}

impl TryFrom<ProtobufElement> for Vec<ProtobufElement> {
    type Error = ProtobufError;

    fn try_from(p: ProtobufElement) -> Result<Self, Self::Error> {
        if let ProtobufElement::Array(v) = p {
            Ok(v)
        } else {
            Err(ProtobufError::NotString)
        }
    }
}

impl TryFrom<ProtobufElement> for ProtobufObject {
    type Error = ProtobufError;

    fn try_from(p: ProtobufElement) -> Result<Self, Self::Error> {
        if let ProtobufElement::Object(v) = p {
            Ok(v)
        } else if let ProtobufElement::Bytes(v) = p {
            decode(&mut v.as_slice())
        } else {
            Err(ProtobufError::NotObject)
        }
    }
}

impl From<isize> for ProtobufElement {
    fn from(value: isize) -> Self {
        Self::Integer(value)
    }
}

impl From<i64> for ProtobufElement {
    fn from(value: i64) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u64> for ProtobufElement {
    fn from(value: u64) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<i32> for ProtobufElement {
    fn from(value: i32) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u32> for ProtobufElement {
    fn from(value: u32) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<i16> for ProtobufElement {
    fn from(value: i16) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u16> for ProtobufElement {
    fn from(value: u16) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<i8> for ProtobufElement {
    fn from(value: i8) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u8> for ProtobufElement {
    fn from(value: u8) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<f32> for ProtobufElement {
    fn from(value: f32) -> Self {
        Self::Double(value as f64)
    }
}

impl From<f64> for ProtobufElement {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<&str> for ProtobufElement {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl From<String> for ProtobufElement {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Vec<u8>> for ProtobufElement {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl<const N: usize> From<[u8; N]> for ProtobufElement {
    fn from(value: [u8; N]) -> Self {
        Self::Bytes(value.to_vec())
    }
}

impl From<&[u8]> for ProtobufElement {
    fn from(value: &[u8]) -> Self {
        Self::Bytes(value.to_vec())
    }
}

impl From<Vec<ProtobufElement>> for ProtobufElement {
    fn from(value: Vec<ProtobufElement>) -> Self {
        Self::Array(value)
    }
}

impl<const N: usize> From<[ProtobufElement; N]> for ProtobufElement {
    fn from(value: [ProtobufElement; N]) -> Self {
        Self::Array(value.to_vec())
    }
}

impl From<ProtobufObject> for ProtobufElement {
    fn from(value: ProtobufObject) -> Self {
        Self::Object(value)
    }
}

#[derive(Debug, Clone)]
pub struct ProtobufObject(HashMap<u32, ProtobufElement>);

impl ProtobufObject {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn into_inner(self) -> HashMap<u32, ProtobufElement> {
        self.0
    }

    pub fn try_remove(&mut self, k: &u32) -> Result<ProtobufElement, ProtobufError> {
        self.0.remove(k).ok_or(ProtobufError::ItemNotFound(*k))
    }

    pub fn try_get(&mut self, k: &u32) -> Result<&ProtobufElement, ProtobufError> {
        self.0.get(k).ok_or(ProtobufError::ItemNotFound(*k))
    }
}

impl From<Vec<(u32, ProtobufElement)>> for ProtobufObject {
    fn from(arr: Vec<(u32, ProtobufElement)>) -> Self {
        Self(arr.into_iter().collect::<HashMap<_, _>>())
    }
}

impl<const N: usize> From<[(u32, ProtobufElement); N]> for ProtobufObject {
    fn from(arr: [(u32, ProtobufElement); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

impl<const N: usize> From<[ProtobufElement; N]> for ProtobufObject {
    fn from(arr: [ProtobufElement; N]) -> Self {
        Self(
            arr.into_iter()
                .enumerate()
                .map(|(i, e)| (i as u32, e))
                .collect::<HashMap<_, _>>(),
        )
    }
}

impl std::ops::DerefMut for ProtobufObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Deref for ProtobufObject {
    type Target = HashMap<u32, ProtobufElement>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn encode_element(
    writer: &mut CodedOutputStream,
    tag: u32,
    element: &ProtobufElement,
) -> Result<(), ProtobufError> {
    match element {
        ProtobufElement::Null => {}
        ProtobufElement::Integer(value) => {
            if *value < 0 {
                writer.write_sint64(tag, *value as i64)?;
            } else {
                writer.write_int64(tag, *value as i64)?;
            }
        }
        ProtobufElement::Double(value) => {
            writer.write_double(tag, *value)?;
        }
        ProtobufElement::Bytes(value) => {
            writer.write_bytes(tag, &value)?;
        }
        ProtobufElement::String(value) => {
            writer.write_string(tag, value)?;
        }
        ProtobufElement::Array(value) => {
            value
                .iter()
                .try_for_each(|e| encode_element(writer, tag, e))?;
        }
        ProtobufElement::Object(value) => {
            writer.write_bytes(tag, &encode_with_capacity(value, 100)?)?;
        }
    }

    Ok(())
}

/// debug 模式下，HashMap 会按照 key(u8) 排序后生成 JceObject，以方便配合 nodejs debug
#[cfg(debug_assertions)]
fn encode_object(
    writer: &mut CodedOutputStream,
    object: &ProtobufObject,
) -> Result<(), ProtobufError> {
    let mut list = object.iter().collect::<Vec<_>>();
    list.sort_by(|a, b| (*a.0).cmp(b.0));

    list.into_iter()
        .try_for_each(|(tag, element)| encode_element(writer, *tag, element))
}

#[cfg(not(debug_assertions))]
fn encode_object(
    writer: &mut CodedOutputStream,
    object: &ProtobufObject,
) -> Result<(), ProtobufError> {
    object
        .iter()
        .try_for_each(|(tag, element)| encode_element(writer, *tag, element))
}

pub fn encode_stream<W>(writer: &mut W, object: &ProtobufObject) -> Result<(), ProtobufError>
where
    W: Write,
{
    let mut coded_stream = CodedOutputStream::new(writer);
    encode_object(&mut coded_stream, object)
}

pub fn encode_with_capacity(
    object: &ProtobufObject,
    capacity: usize,
) -> Result<Vec<u8>, ProtobufError> {
    let mut buf = Vec::with_capacity(capacity);
    let mut coded_stream = CodedOutputStream::new(&mut buf);
    encode_object(&mut coded_stream, object)?;
    drop(coded_stream);

    Ok(buf)
}

pub fn encode(object: &ProtobufObject) -> Result<Vec<u8>, ProtobufError> {
    encode_with_capacity(object, 1024)
}

fn decode_object(reader: &mut CodedInputStream) -> Result<ProtobufObject, ProtobufError> {
    let mut result = ProtobufObject::with_capacity(20);

    while !reader.eof()? {
        let k = reader.read_uint32()?;
        let tag = k >> 3;
        let typee = k & 0b111;

        let value = match typee {
            0 => ProtobufElement::Integer(reader.read_int64()? as isize),
            1 => ProtobufElement::Integer(reader.read_fixed64()? as isize),
            2 => {
                let buf = reader.read_bytes()?;
                ProtobufElement::Bytes(buf)
            }
            5 => ProtobufElement::Integer(reader.read_fixed32()? as isize),
            _ => return Err(ProtobufError::InvalidType(typee)),
        };

        if let Some(e) = result.get_mut(&tag) {
            if let ProtobufElement::Array(list) = e {
                list.push(value);
            } else {
                let e = result.remove(&tag).unwrap();
                let value = ProtobufElement::Array(vec![e]);
                result.insert(tag, value);
            };
        } else {
            result.insert(tag, value);
        }
    }

    Ok(result)
}

pub fn decode<R: Read>(reader: &mut R) -> Result<ProtobufObject, ProtobufError> {
    let mut reader = CodedInputStream::new(reader);
    decode_object(&mut reader)
}

#[cfg(test)]
mod test {
    use crate::{core::io::WriteExt, error::Error};

    use super::{encode, ProtobufElement, ProtobufObject};

    #[test]
    fn test() -> Result<(), Error> {
        let mut buf = Vec::with_capacity(9);
        buf.write_u32(324372432)?;
        buf.write_bytes([0x00, 0x00, 0x01, 0x9e, 0x39])?;

        println!("{:02x?} {}", buf, buf.len());

        let encoded = encode(&ProtobufObject::from([
            (1, ProtobufElement::Integer(1152)),
            (2, ProtobufElement::Integer(9)),
            (3, ProtobufElement::String("242423424".to_string())),
            (4, ProtobufElement::Bytes(buf)),
            (5, ProtobufElement::Double(213.435)),
            (
                6,
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::Integer(2)),
                    (2, ProtobufElement::String("sdf".to_string())),
                ])),
            ),
            (
                7,
                ProtobufElement::Array(vec![
                    ProtobufElement::Integer(123),
                    ProtobufElement::String("345".to_string()),
                ]),
            ),
        ]))?;

        println!("{:02x?} {}", encoded, encoded.len());

        Ok(())
    }
}
