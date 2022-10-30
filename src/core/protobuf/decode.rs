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

/// Protobuf 解码元素
#[derive(Debug, Clone)]
pub enum Element {
    Integer(i128),
    Double(f64),
    Bytes(Vec<u8>),
    String(String),
    Array(Vec<Element>),
    Object(Object),
}

impl TryFrom<Element> for i128 {
    type Error = DecodeError;

    fn try_from(p: Element) -> Result<Self, Self::Error> {
        if let Element::Integer(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotInteger)
        }
    }
}

impl TryFrom<Element> for f64 {
    type Error = DecodeError;

    fn try_from(p: Element) -> Result<Self, Self::Error> {
        if let Element::Double(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotDouble)
        }
    }
}

impl TryFrom<Element> for Vec<u8> {
    type Error = DecodeError;

    fn try_from(p: Element) -> Result<Self, Self::Error> {
        if let Element::Bytes(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotBytes)
        }
    }
}

impl TryFrom<Element> for String {
    type Error = DecodeError;

    fn try_from(p: Element) -> Result<Self, Self::Error> {
        if let Element::String(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotString)
        }
    }
}

impl TryFrom<Element> for Vec<Element> {
    type Error = DecodeError;

    fn try_from(p: Element) -> Result<Self, Self::Error> {
        if let Element::Array(v) = p {
            Ok(v)
        } else {
            Err(DecodeError::NotString)
        }
    }
}

impl TryFrom<Element> for Object {
    type Error = DecodeError;

    fn try_from(p: Element) -> Result<Self, Self::Error> {
        if let Element::Object(v) = p {
            Ok(v)
        } else if let Element::Bytes(v) = p {
            Object::decode(&mut v.as_slice())
        } else {
            Err(DecodeError::NotObject)
        }
    }
}

/// Protobuf 解码元素容器
#[derive(Debug, Clone)]
pub struct Object(HashMap<u32, Element>);

impl Object {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn into_inner(self) -> HashMap<u32, Element> {
        self.0
    }

    pub fn try_remove(&mut self, k: &u32) -> Result<Element, DecodeError> {
        self.0.remove(k).ok_or(DecodeError::ItemNotFound(*k))
    }

    pub fn try_get(&mut self, k: &u32) -> Result<&Element, DecodeError> {
        self.0.get(k).ok_or(DecodeError::ItemNotFound(*k))
    }

    pub fn decode<R: Read>(reader: &mut R) -> Result<Object, DecodeError> {
        let mut stream = CodedInputStream::new(reader);
        Object::decode_object(&mut stream)
    }

    fn decode_object(stream: &mut CodedInputStream) -> Result<Object, DecodeError> {
        let mut result = Object::with_capacity(20);
    
        while !stream.eof()? {
            let k = stream.read_uint32()?;
            let tag = k >> 3;
            let typee = k & 0b111;
    
            let value = match typee {
                0 => Element::Integer(stream.read_int64()? as i128),
                1 => Element::Integer(stream.read_fixed64()? as i128),
                2 => {
                    let buf = stream.read_bytes()?;
                    Element::Bytes(buf)
                }
                5 => Element::Integer(stream.read_fixed32()? as i128),
                _ => return Err(DecodeError::InvalidType(typee)),
            };
    
            if let Some(e) = result.get_mut(&tag) {
                if let Element::Array(list) = e {
                    list.push(value);
                } else {
                    let e = result.remove(&tag).unwrap();
                    let value = Element::Array(vec![e]);
                    result.insert(tag, value);
                };
            } else {
                result.insert(tag, value);
            }
        }
    
        Ok(result)
    }
}

impl std::ops::DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Deref for Object {
    type Target = HashMap<u32, Element>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
