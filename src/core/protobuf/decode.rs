use std::{collections::HashMap, fmt::Display, io::Read};

use protobuf::CodedInputStream;

#[derive(Debug)]
pub enum DecodeError {
    ProtobufError(protobuf::Error),
    InvalidType(u32),
    ItemNotFound(u32),
    NotInteger,
    NotDouble,
    NotBytes,
    NotString,
    NotArray,
    NotObject,
}

impl std::error::Error for DecodeError {}

impl Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::ProtobufError(err) => err.fmt(f),
            DecodeError::InvalidType(typee) => {
                f.write_fmt(format_args!("invalid type: {}.", typee))
            }
            DecodeError::ItemNotFound(e) => f.write_fmt(format_args!("item not found: {}.", *e)),
            DecodeError::NotInteger => f.write_str("type of this protobuf element not Integer"),
            DecodeError::NotDouble => f.write_str("type of this protobuf element not Double"),
            DecodeError::NotBytes => f.write_str("type of this protobuf element not Bytes"),
            DecodeError::NotString => f.write_str("type of this protobuf element not String"),
            DecodeError::NotArray => f.write_str("type of this protobuf element not Array"),
            DecodeError::NotObject => {
                f.write_str("type of this protobuf element not ProtobufObject")
            }
        }
    }
}

impl From<protobuf::Error> for DecodeError {
    fn from(err: protobuf::Error) -> Self {
        Self::ProtobufError(err)
    }
}

pub trait DecodeProtobuf {
    fn decode_protobuf(&mut self) -> Result<DecodedObject, DecodeError>;
}

impl<R: Read> DecodeProtobuf for R {
    fn decode_protobuf(&mut self) -> Result<DecodedObject, DecodeError> {
        let mut reader = CodedInputStream::new(self);
        decode_object(&mut reader)
    }
}

fn decode_object(reader: &mut CodedInputStream) -> Result<DecodedObject, DecodeError> {
    let mut result = DecodedObject::with_capacity(20);

    while !reader.eof()? {
        let k = reader.read_uint32()?;
        let tag = k >> 3;
        let typee = k & 0b111;

        let value = match typee {
            0 => DecodedElement::Integer(reader.read_int64()? as isize),
            1 => DecodedElement::Integer(reader.read_fixed64()? as isize),
            2 => {
                let buf = reader.read_bytes()?;
                DecodedElement::Bytes(buf)
            }
            5 => DecodedElement::Integer(reader.read_fixed32()? as isize),
            _ => return Err(DecodeError::InvalidType(typee)),
        };

        if let Some(e) = result.get_mut(&tag) {
            if let DecodedElement::Array(list) = e {
                list.push(value);
            } else {
                let e = result.remove(&tag).unwrap();
                let value = DecodedElement::Array(vec![e]);
                result.insert(tag, value);
            };
        } else {
            result.insert(tag, value);
        }
    }

    Ok(result)
}

/// Protobuf 解码元素
#[derive(Debug, Clone)]
pub enum DecodedElement {
    Null,
    Integer(isize),
    Double(f64),
    Bytes(Vec<u8>),
    String(String),
    Array(Vec<DecodedElement>),
    Object(DecodedObject),
}

impl TryFrom<DecodedElement> for isize {
    type Error = DecodeError;

    fn try_from(p: DecodedElement) -> Result<Self, Self::Error> {
        if let DecodedElement::Integer(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotInteger)
        }
    }
}

impl TryFrom<DecodedElement> for f64 {
    type Error = DecodeError;

    fn try_from(p: DecodedElement) -> Result<Self, Self::Error> {
        if let DecodedElement::Double(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotDouble)
        }
    }
}

impl TryFrom<DecodedElement> for Vec<u8> {
    type Error = DecodeError;

    fn try_from(p: DecodedElement) -> Result<Self, Self::Error> {
        if let DecodedElement::Bytes(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotBytes)
        }
    }
}

impl TryFrom<DecodedElement> for String {
    type Error = DecodeError;

    fn try_from(p: DecodedElement) -> Result<Self, Self::Error> {
        if let DecodedElement::String(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotString)
        }
    }
}

impl TryFrom<DecodedElement> for Vec<DecodedElement> {
    type Error = DecodeError;

    fn try_from(p: DecodedElement) -> Result<Self, Self::Error> {
        if let DecodedElement::Array(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotString)
        }
    }
}

impl TryFrom<DecodedElement> for DecodedObject {
    type Error = DecodeError;

    fn try_from(p: DecodedElement) -> Result<Self, Self::Error> {
        if let DecodedElement::Object(v) = p {
            Ok(v)
        } else if let DecodedElement::Bytes(v) = p {
            v.as_slice().decode_protobuf()
        } else {
            Err(DecodeError::NotObject)
        }
    }
}

impl From<isize> for DecodedElement {
    fn from(value: isize) -> Self {
        Self::Integer(value)
    }
}

impl From<i64> for DecodedElement {
    fn from(value: i64) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u64> for DecodedElement {
    fn from(value: u64) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<i32> for DecodedElement {
    fn from(value: i32) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u32> for DecodedElement {
    fn from(value: u32) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<i16> for DecodedElement {
    fn from(value: i16) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u16> for DecodedElement {
    fn from(value: u16) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<i8> for DecodedElement {
    fn from(value: i8) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<u8> for DecodedElement {
    fn from(value: u8) -> Self {
        Self::Integer(value as isize)
    }
}

impl From<f32> for DecodedElement {
    fn from(value: f32) -> Self {
        Self::Double(value as f64)
    }
}

impl From<f64> for DecodedElement {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<&str> for DecodedElement {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl From<String> for DecodedElement {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Vec<u8>> for DecodedElement {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl<const N: usize> From<[u8; N]> for DecodedElement {
    fn from(value: [u8; N]) -> Self {
        Self::Bytes(value.to_vec())
    }
}

impl From<&[u8]> for DecodedElement {
    fn from(value: &[u8]) -> Self {
        Self::Bytes(value.to_vec())
    }
}

impl From<Vec<DecodedElement>> for DecodedElement {
    fn from(value: Vec<DecodedElement>) -> Self {
        Self::Array(value)
    }
}

impl<const N: usize> From<[DecodedElement; N]> for DecodedElement {
    fn from(value: [DecodedElement; N]) -> Self {
        Self::Array(value.to_vec())
    }
}

impl From<DecodedObject> for DecodedElement {
    fn from(value: DecodedObject) -> Self {
        Self::Object(value)
    }
}

/// Protobuf 解码元素容器
#[derive(Debug, Clone)]
pub struct DecodedObject(HashMap<u32, DecodedElement>);

impl DecodedObject {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn into_inner(self) -> HashMap<u32, DecodedElement> {
        self.0
    }

    pub fn try_remove(&mut self, k: &u32) -> Result<DecodedElement, DecodeError> {
        self.0.remove(k).ok_or(DecodeError::ItemNotFound(*k))
    }

    pub fn try_get(&mut self, k: &u32) -> Result<&DecodedElement, DecodeError> {
        self.0.get(k).ok_or(DecodeError::ItemNotFound(*k))
    }
}

// impl From<Vec<(u32, DecodedElement)>> for DecodedObject {
//     fn from(arr: Vec<(u32, DecodedElement)>) -> Self {
//         Self(arr.into_iter().collect::<HashMap<_, _>>())
//     }
// }

impl<const N: usize> From<[(u32, DecodedElement); N]> for DecodedObject {
    fn from(arr: [(u32, DecodedElement); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

impl<const N: usize> From<[DecodedElement; N]> for DecodedObject {
    fn from(arr: [DecodedElement; N]) -> Self {
        Self(
            arr.into_iter()
                .enumerate()
                .map(|(i, e)| (i as u32, e))
                .collect::<HashMap<_, _>>(),
        )
    }
}

impl std::ops::DerefMut for DecodedObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Deref for DecodedObject {
    type Target = HashMap<u32, DecodedElement>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
