use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt::Display,
    io::{Read, Write},
};

use crate::error::{Error, ErrorKind};

use super::io::WriteExt;

#[derive(Debug)]
pub enum JceError {
    NotFloat,
    NotDouble,
    NotInt64,
    NotInt32,
    NotInt16,
    NotInt8,
    NotString,
    NotSimpleList,
    TagOverflowed(usize),
    DecodeError,
    TypeNotMatched(u8),
    ItemNotFound(u8),
}

impl std::error::Error for JceError {}

impl Display for JceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JceError::NotFloat => f.write_str("type of this jce element not Float"),
            JceError::NotDouble => f.write_str("type of this jce element not Double"),
            JceError::NotInt64 => f.write_str("type of this jce element not SignedInteger64"),
            JceError::NotInt32 => f.write_str("type of this jce element not SignedInteger32"),
            JceError::NotInt16 => f.write_str("type of this jce element not SignedInteger16"),
            JceError::NotInt8 => f.write_str("type of this jce element not SignedInteger8"),
            JceError::NotString => f.write_str("type of this jce element not String"),
            JceError::NotSimpleList => f.write_str("type of this jce element not SimpleList"),
            JceError::TagOverflowed(tag) => f.write_fmt(format_args!(
                "jce tag overflowed, max: 255, current: {}",
                *tag
            )),
            JceError::DecodeError => f.write_str("decode error"),
            JceError::TypeNotMatched(e) => f.write_fmt(format_args!("type not matched: {}", *e)),
            JceError::ItemNotFound(e) => f.write_fmt(format_args!("item not found: {}.", *e)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Type {
    Int8 = 0,
    Int16 = 1,
    Int32 = 2,
    Int64 = 3,
    Float = 4,
    Double = 5,
    String1 = 6,
    String4 = 7,
    Map = 8,
    List = 9,
    StructBegin = 10,
    StructEnd = 11,
    Zero = 12,
    SimpleList = 13,
}

impl TryFrom<u8> for Type {
    type Error = JceError;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        if byte == 0 {
            Ok(Self::Int8)
        } else if byte == 1 {
            Ok(Self::Int16)
        } else if byte == 2 {
            Ok(Self::Int32)
        } else if byte == 3 {
            Ok(Self::Int64)
        } else if byte == 4 {
            Ok(Self::Float)
        } else if byte == 5 {
            Ok(Self::Double)
        } else if byte == 6 {
            Ok(Self::String1)
        } else if byte == 7 {
            Ok(Self::String4)
        } else if byte == 8 {
            Ok(Self::Map)
        } else if byte == 9 {
            Ok(Self::List)
        } else if byte == 10 {
            Ok(Self::StructBegin)
        } else if byte == 11 {
            Ok(Self::StructEnd)
        } else if byte == 12 {
            Ok(Self::Zero)
        } else if byte == 13 {
            Ok(Self::SimpleList)
        } else {
            Err(JceError::TypeNotMatched(byte))
        }
    }
}

impl From<&JceElement> for Type {
    fn from(element: &JceElement) -> Self {
        match element {
            JceElement::Zero => Self::Zero,
            JceElement::Int8(_) => Self::Int8,
            JceElement::Int16(_) => Self::Int16,
            JceElement::Int32(_) => Self::Int32,
            JceElement::Int64(_) => Self::Int64,
            JceElement::Float(_) => Self::Float,
            JceElement::Double(_) => Self::Double,
            JceElement::String1(_) => Self::String1,
            JceElement::String4(_) => Self::String4,
            JceElement::SimpleList(_) => Self::SimpleList,
            JceElement::List(_) => Self::List,
            JceElement::Map(_) => Self::Map,
            JceElement::StructBegin(_) => Self::StructBegin,
            JceElement::StructEnd => Self::StructEnd,
        }
    }
}

#[derive(Debug, Clone)]
pub enum JceElement {
    Zero,
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Float(f32),
    Double(f64),
    String1(String),
    String4(String),
    SimpleList(Vec<u8>),
    List(Vec<JceElement>),
    Map(HashMap<String, JceElement>),
    StructBegin(JceObject),
    StructEnd,
}

impl From<i64> for JceElement {
    fn from(value: i64) -> Self {
        if value == 0 {
            Self::Zero
        } else if value >= -0x80 && value <= 0x7f {
            Self::Int8(value as i8)
        } else if value >= -0x8000 && value <= 0x7fff {
            Self::Int16(value as i16)
        } else if value >= -0x80000000 && value <= 0x7fffffff {
            Self::Int32(value as i32)
        } else {
            Self::Int64(value)
        }
    }
}

impl From<f32> for JceElement {
    fn from(value: f32) -> Self {
        Self::Float(value)
    }
}

impl From<f64> for JceElement {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<&str> for JceElement {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl From<String> for JceElement {
    fn from(value: String) -> Self {
        if value.len() <= 0xff {
            Self::String1(value)
        } else {
            Self::String4(value)
        }
    }
}

impl<const N: usize> From<[(String, JceElement); N]> for JceElement {
    fn from(arr: [(String, JceElement); N]) -> Self {
        Self::Map(HashMap::from(arr))
    }
}

impl<const N: usize> From<[(&'static str, JceElement); N]> for JceElement {
    fn from(arr: [(&'static str, JceElement); N]) -> Self {
        let iter = arr.into_iter().map(|(k, v)| (k.to_string(), v));
        Self::Map(HashMap::from_iter(iter))
    }
}

impl From<HashMap<String, JceElement>> for JceElement {
    fn from(value: HashMap<String, JceElement>) -> Self {
        Self::Map(value)
    }
}

impl From<Vec<u8>> for JceElement {
    fn from(value: Vec<u8>) -> Self {
        Self::SimpleList(value)
    }
}

impl<const N: usize> From<[u8; N]> for JceElement {
    fn from(value: [u8; N]) -> Self {
        Self::SimpleList(value.to_vec())
    }
}

impl From<Vec<JceElement>> for JceElement {
    fn from(value: Vec<JceElement>) -> Self {
        Self::List(value)
    }
}

impl<const N: usize> From<[JceElement; N]> for JceElement {
    fn from(value: [JceElement; N]) -> Self {
        Self::List(value.to_vec())
    }
}

impl<const N: usize> From<[(u8, JceElement); N]> for JceElement {
    fn from(arr: [(u8, JceElement); N]) -> Self {
        Self::StructBegin(JceObject::from(arr))
    }
}

impl From<JceObject> for JceElement {
    fn from(value: JceObject) -> Self {
        Self::StructBegin(value)
    }
}

impl TryFrom<JceElement> for i64 {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::Zero => Ok(0),
            JceElement::Int8(len) => Ok(len as i64),
            JceElement::Int16(len) => Ok(len as i64),
            JceElement::Int32(len) => Ok(len as i64),
            JceElement::Int64(len) => Ok(len),
            _ => Err(JceError::NotInt64),
        }
    }
}

impl TryFrom<JceElement> for i32 {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::Zero => Ok(0),
            JceElement::Int8(len) => Ok(len as i32),
            JceElement::Int16(len) => Ok(len as i32),
            JceElement::Int32(len) => Ok(len),
            _ => Err(JceError::NotInt32),
        }
    }
}

impl TryFrom<JceElement> for i16 {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::Zero => Ok(0),
            JceElement::Int8(len) => Ok(len as i16),
            JceElement::Int16(len) => Ok(len),
            _ => Err(JceError::NotInt16),
        }
    }
}

impl TryFrom<JceElement> for i8 {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::Zero => Ok(0),
            JceElement::Int8(len) => Ok(len),
            _ => Err(JceError::NotInt8),
        }
    }
}

impl TryFrom<JceElement> for f32 {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::Zero => Ok(0.0),
            JceElement::Float(len) => Ok(len),
            _ => Err(JceError::NotFloat),
        }
    }
}

impl TryFrom<JceElement> for f64 {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::Zero => Ok(0.0),
            JceElement::Float(len) => Ok(len as f64),
            JceElement::Double(len) => Ok(len),
            _ => Err(JceError::NotDouble),
        }
    }
}

impl TryFrom<JceElement> for String {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::String1(str) => Ok(str),
            JceElement::String4(str) => Ok(str),
            _ => Err(JceError::NotString),
        }
    }
}

impl TryFrom<JceElement> for Vec<u8> {
    type Error = JceError;

    fn try_from(value: JceElement) -> Result<Self, Self::Error> {
        match value {
            JceElement::SimpleList(buf) => Ok(buf),
            _ => Err(JceError::NotSimpleList),
        }
    }
}

impl Display for JceElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JceElement::Zero => f.write_str("0"),
            JceElement::Int8(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::Int16(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::Int32(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::Int64(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::Float(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::Double(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::String1(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::String4(value) => f.write_fmt(format_args!("{}", value)),
            JceElement::SimpleList(value) => f.write_fmt(format_args!("<Buffer {:02x?}>", value)),
            JceElement::List(value) => f.write_fmt(format_args!(
                "[ {} ]",
                value
                    .iter()
                    .map(|element| element.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            )),
            JceElement::Map(value) => {
                let str = value
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_string()))
                    .collect::<Vec<String>>()
                    .join(", ");

                f.write_fmt(format_args!("{{ {} }}", str))
            }
            JceElement::StructBegin(value) => {
                let mut entries = value.iter().collect::<Vec<(&u8, &JceElement)>>();
                entries.sort_by(|a, b| (*a.0).cmp(b.0));
                let result = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_string()))
                    .collect::<Vec<String>>()
                    .join(", ");

                f.write_fmt(format_args!("{{ {} }}", result))
            }
            JceElement::StructEnd => Ok(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct JceObject(HashMap<u8, JceElement>);

impl JceObject {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn raw(self) -> HashMap<u8, JceElement> {
        self.0
    }

    pub fn try_remove(&mut self, k: &u8) -> Result<JceElement, JceError> {
        self.0.remove(k).ok_or(JceError::ItemNotFound(*k))
    }

    pub fn try_get(&mut self, k: &u8) -> Result<&JceElement, JceError> {
        self.0.get(k).ok_or(JceError::ItemNotFound(*k))
    }
}

impl From<JceElement> for JceObject {
    fn from(arr: JceElement) -> Self {
        Self(HashMap::from([(0, arr)]))
    }
}

impl<const N: usize> From<[(u8, JceElement); N]> for JceObject {
    fn from(arr: [(u8, JceElement); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

impl<const N: usize> TryFrom<[Option<JceElement>; N]> for JceObject {
    type Error = JceError;

    fn try_from(arr: [Option<JceElement>; N]) -> Result<Self, Self::Error> {
        let iterator = arr.into_iter();
        let mut map = HashMap::with_capacity(iterator.len());
        iterator.enumerate().try_for_each(|(tag, ele)| {
            if tag <= u8::MAX as usize {
                if let Some(ele) = ele {
                    map.insert(tag as u8, ele);
                }
                Ok(())
            } else {
                Err(JceError::TagOverflowed(tag))
            }
        })?;

        Ok(Self(map))
    }
}

impl std::ops::Deref for JceObject {
    type Target = HashMap<u8, JceElement>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for JceObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn read_head<R>(stream: &mut R) -> Result<(u8, Type), Error>
where
    R: Read,
{
    let mut buf = [0; 1];
    stream.read_exact(&mut buf)?;

    let head = buf[0];
    let typee = Type::try_from(head & 0xf)?;
    let mut tag = (head & 0xf0) >> 4;
    if tag == 0xf {
        stream.read_exact(&mut buf)?;
        tag = buf[0];
    }

    Ok((tag, typee))
}

fn read_body<R>(stream: &mut R, typee: Type) -> Result<JceElement, Error>
where
    R: Read,
{
    match typee {
        Type::Int8 => {
            let mut buf = [0; 1];
            stream.read_exact(&mut buf)?;
            Ok(JceElement::Int8(buf[0] as i8))
        }
        Type::Int16 => {
            let mut buf = [0; 2];
            stream.read_exact(&mut buf)?;
            Ok(JceElement::Int16(i16::from_be_bytes(buf)))
        }
        Type::Int32 => {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf)?;
            Ok(JceElement::Int32(i32::from_be_bytes(buf)))
        }
        Type::Int64 => {
            let mut buf = [0; 8];
            stream.read_exact(&mut buf)?;
            Ok(JceElement::Int64(i64::from_be_bytes(buf)))
        }
        Type::Float => {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf)?;
            Ok(JceElement::Float(f32::from_be_bytes(buf)))
        }
        Type::Double => {
            let mut buf = [0; 8];
            stream.read_exact(&mut buf)?;
            Ok(JceElement::Double(f64::from_be_bytes(buf)))
        }
        Type::String1 => {
            let mut buf = [0; 1];
            stream.read_exact(&mut buf)?;

            let len = buf[0] as usize;
            if len > 0 {
                let mut buf = vec![0; len];
                stream.read_exact(&mut buf)?;
                Ok(JceElement::String1(String::from_utf8(buf)?))
            } else {
                Ok(JceElement::String1(String::new()))
            }
        }
        Type::String4 => {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf)?;

            let len = u32::from_be_bytes(buf) as usize;
            if len > 0 {
                let mut buf = vec![0; len];
                stream.read_exact(&mut buf)?;
                Ok(JceElement::String4(String::from_utf8(buf)?))
            } else {
                Ok(JceElement::String4(String::new()))
            }
        }
        Type::Map => {
            let (_, element) = read_element(stream)?;
            let mut len = i64::try_from(element)? as usize;
            let mut map = HashMap::new();
            while len > 0 {
                let (_, element) = read_element(stream)?;
                let key = String::try_from(element)?;
                let (_, element) = read_element(stream)?;
                map.insert(key, element);
                len -= 1;
            }
            Ok(JceElement::Map(map))
        }
        Type::List => {
            let (_, element) = read_element(stream)?;
            let mut len = i64::try_from(element)? as usize;
            let mut list = Vec::with_capacity(len);
            while len > 0 {
                list.push(read_element(stream)?.1);
                len -= 1;
            }

            Ok(JceElement::List(list))
        }
        Type::StructBegin => read_struct(stream),
        Type::StructEnd => Ok(JceElement::StructEnd),
        Type::Zero => Ok(JceElement::Zero),
        Type::SimpleList => {
            read_head(stream)?;
            let (_, element) = read_element(stream)?;
            let len = i64::try_from(element)? as usize;
            if len > 0 {
                let mut buf = vec![0; len];
                stream.read_exact(&mut buf)?;
                Ok(JceElement::SimpleList(buf))
            } else {
                Ok(JceElement::SimpleList(vec![]))
            }
        }
    }
}

fn read_struct<R>(stream: &mut R) -> Result<JceElement, Error>
where
    R: Read,
{
    let mut result = JceObject::new();

    loop {
        let (tag, element) = read_element(stream)?;
        match &element {
            JceElement::StructEnd => break,
            _ => result.insert(tag, element),
        };
    }

    Ok(JceElement::StructBegin(result))
}

fn read_element<R>(stream: &mut R) -> Result<(u8, JceElement), Error>
where
    R: Read,
{
    let (tag, typee) = read_head(stream)?;
    let element = read_body(stream, typee)?;
    Ok((tag, element))
}

fn create_head<W>(stream: &mut W, tag: u8, typee: Type) -> Result<(), Error>
where
    W: Write,
{
    let typee = typee as u8;
    if tag < 15 {
        stream.write_bytes([(tag << 4) | typee])?;
    } else {
        stream.write_bytes([0xf0 | typee, tag])?;
    }

    Ok(())
}

fn create_body<W>(stream: &mut W, body: &JceElement) -> Result<(), Error>
where
    W: Write,
{
    match body {
        JceElement::Zero => {}
        JceElement::Int8(value) => stream.write_i8(*value)?,
        JceElement::Int16(value) => stream.write_i16(*value)?,
        JceElement::Int32(value) => stream.write_i32(*value)?,
        JceElement::Int64(value) => stream.write_i64(*value)?,
        JceElement::Float(value) => stream.write_f32(*value)?,
        JceElement::Double(value) => stream.write_f64(*value)?,
        JceElement::String1(value) => {
            stream.write_u8(value.len() as u8)?;
            stream.write_bytes(value)?;
        }
        JceElement::String4(value) => {
            stream.write_u32(value.len() as u32)?;
            stream.write_bytes(value)?;
        }
        JceElement::SimpleList(value) => {
            create_head(stream, 0, Type::Int8)?; // 仅占位，没有实际意义
            create_element(stream, 0, &JceElement::from(value.len() as i64))?;
            stream.write_bytes(value)?;
        }
        JceElement::List(value) => {
            create_element(stream, 0, &JceElement::from(value.len() as i64))?;
            value
                .iter()
                .try_for_each(|element| create_element(stream, 0, element))?;
        }
        JceElement::Map(value) => {
            create_element(stream, 0, &JceElement::from(value.len() as i64))?;
            value.iter().try_for_each(|(k, v)| {
                create_element(stream, 0, &JceElement::from(k.as_str()))?;
                create_element(stream, 1, v)
            })?;
        }
        JceElement::StructBegin(value) => {
            encode_stream(stream, value)?;
            create_head(stream, 0, Type::StructEnd)?;
        }
        JceElement::StructEnd => {
            // JceElement::StructBegin 负责一并添加 JceElement::StructEnd
        }
    }

    Ok(())
}

fn create_element<W>(stream: &mut W, tag: u8, element: &JceElement) -> Result<(), Error>
where
    W: Write,
{
    create_head(stream, tag, Type::from(element))?;
    create_body(stream, element)
}

/// 调用此函数进行jce解码
pub fn decode<R>(source: &mut R) -> Result<JceObject, Error>
where
    R: Read,
{
    let mut result = JceObject::new();

    loop {
        let read = read_element(source);
        match read {
            Ok((tag, element)) => {
                result.insert(tag, element);
            }
            Err(error) => match &error.kind() {
                ErrorKind::StdIoError(io_error) => {
                    if let std::io::ErrorKind::UnexpectedEof = io_error.kind() {
                        break;
                    } else {
                        return Err(error);
                    }
                }
                _ => return Err(error),
            },
        }
    }

    Ok(result)
}

pub fn decode_wrapper<R>(source: &mut R) -> Result<JceElement, Error>
where
    R: Read,
{
    let mut wrapper = decode(source)?;
    wrapper
        .remove(&7)
        .ok_or(Error::from(JceError::DecodeError))
        .and_then(|ele| {
            if let JceElement::SimpleList(value) = ele {
                decode(&mut value.as_slice())
            } else {
                Err(Error::from(JceError::DecodeError))
            }
        })
        .and_then(|mut object| object.remove(&0).ok_or(Error::from(JceError::DecodeError)))
        .and_then(|map| {
            if let JceElement::Map(value) = map {
                value
                    .into_values()
                    .next()
                    .ok_or(Error::from(JceError::DecodeError))
            } else {
                Err(Error::from(JceError::DecodeError))
            }
        })
        .and_then(|nested| {
            if let JceElement::Map(value) = nested {
                value
                    .into_values()
                    .next()
                    .ok_or(Error::from(JceError::DecodeError))
            } else {
                Ok(nested)
            }
        })
        .and_then(|nested| {
            if let JceElement::SimpleList(value) = nested {
                decode(&mut value.as_slice())
            } else {
                Err(Error::from(JceError::DecodeError))
            }
        })
        .and_then(|mut object| object.remove(&0).ok_or(Error::from(JceError::DecodeError)))
}

#[cfg(not(debug_assertions))]
pub fn encode<W>(stream: &mut W, object: &JceObject) -> Result<(), Error>
where
    W: Write,
{
    object
        .iter()
        .try_for_each(|(tag, element)| create_element(stream, *tag, element))
}

/// debug 模式下，HashMap 会按照 key(u8) 排序后生成 JceObject，以方便配合 nodejs debug
#[cfg(debug_assertions)]
pub fn encode_stream<W>(stream: &mut W, object: &JceObject) -> Result<(), Error>
where
    W: Write,
{
    let mut a = object.iter().collect::<Vec<(&u8, &JceElement)>>();
    a.sort_by(|a, b| (*a.0).cmp(b.0));
    a.into_iter()
        .try_for_each(|(tag, element)| create_element(stream, *tag, element))
}

pub fn encode_with_capacity(object: &JceObject, capacity: usize) -> Result<Vec<u8>, Error> {
    let mut encoded = Vec::with_capacity(capacity);
    encode_stream(&mut encoded, object)?;
    Ok(encoded)
}

pub fn encode(object: &JceObject) -> Result<Vec<u8>, Error> {
    encode_with_capacity(object, 1024)
}

pub fn encode_nested(object: JceObject) -> Result<Vec<u8>, Error> {
    encode(&JceObject::from([(0, JceElement::StructBegin(object))]))
}

pub fn encode_wrapper<K, V, M>(
    map: M,
    servant: &str,
    func: &str,
    req_id: Option<i64>,
) -> Result<Vec<u8>, Error>
where
    K: Into<String>,
    V: Into<JceElement>,
    M: Into<HashMap<K, V>>,
{
    let map: HashMap<K, V> = map.into();
    // 所有输入的不是 SimpleList 的 JceElement encode 成 SimpleList
    let mut simple_list = HashMap::with_capacity(map.len());
    map.into_iter().try_for_each(|(k, v)| {
        let v = v.into();
        match v {
            JceElement::SimpleList(_) => {
                simple_list.insert(k.into(), v);
                Ok(())
            }
            _ => match encode(&JceObject::from(v)) {
                Ok(encoded) => {
                    simple_list.insert(k.into(), JceElement::SimpleList(encoded));
                    Ok(())
                }
                Err(err) => Err(err),
            },
        }
    })?;

    // 然后再生成 Map
    let mut pre_encoded = Vec::<u8>::with_capacity(100);
    encode_stream(
        &mut pre_encoded,
        &JceObject::try_from([Some(JceElement::Map(simple_list))])?,
    )?;

    encode_with_capacity(
        &JceObject::try_from([
            None,
            Some(JceElement::from(3)),
            Some(JceElement::from(0)),
            Some(JceElement::from(0)),
            Some(JceElement::from(req_id.unwrap_or(0))),
            Some(JceElement::from(servant)),
            Some(JceElement::from(func)),
            Some(JceElement::from(pre_encoded)),
            Some(JceElement::from(0)),
            Some(JceElement::from(HashMap::new())),
            Some(JceElement::from(HashMap::new())),
        ])?,
        1000,
    )
}

#[cfg(test)]
mod test {
    use crate::core::{
        helper::current_unix_timestamp_as_secs,
        jce,
        protobuf::{self, ProtobufElement, ProtobufObject},
    };

    use super::*;

    #[test]
    fn test_register() {
        let logout = false;
        let pb_buf = protobuf::encode(&ProtobufObject::from([(
            1,
            ProtobufElement::from([
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(46)),
                    (2, ProtobufElement::from(current_unix_timestamp_as_secs())),
                ])),
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(283)),
                    (2, ProtobufElement::from(0)),
                ])),
            ]),
        )]))
        .unwrap();

        assert_eq!(17, pb_buf.len());

        let svc_req_register = jce::encode_nested(
            JceObject::try_from([
                Some(JceElement::from(640279992)),
                Some(JceElement::from(logout.then_some(0).unwrap_or(7))),
                Some(JceElement::from(0)),
                Some(JceElement::from("")),
                Some(JceElement::from(logout.then_some(21).unwrap_or(11))),
                Some(JceElement::from(0)),
                Some(JceElement::from(0)),
                Some(JceElement::from(0)),
                Some(JceElement::from(0)),
                Some(JceElement::from(0)),
                Some(JceElement::from(logout.then_some(44).unwrap_or(0))),
                Some(JceElement::from(29)),
                Some(JceElement::from(1)),
                Some(JceElement::from("")),
                Some(JceElement::from(0)),
                None,
                Some(JceElement::from(rand::random::<[u8; 16]>())),
                Some(JceElement::from(2052)),
                Some(JceElement::from(0)),
                Some(JceElement::from("Konata 2020")),
                Some(JceElement::from("Konata 2020")),
                Some(JceElement::from("10")),
                Some(JceElement::from(1)),
                Some(JceElement::from(0)),
                Some(JceElement::from(0)),
                None,
                Some(JceElement::from(0)),
                Some(JceElement::from(0)),
                Some(JceElement::from("")),
                Some(JceElement::from(0)),
                Some(JceElement::from("OICQX")),
                Some(JceElement::from("OICQX")),
                Some(JceElement::from("")),
                Some(JceElement::from(pb_buf)),
                Some(JceElement::from(0)),
                None,
                Some(JceElement::from(0)),
                None,
                Some(JceElement::from(1000)),
                Some(JceElement::from(98)),
            ])
            .unwrap(),
        )
        .unwrap();

        assert_eq!(155, svc_req_register.len());
    }

    #[test]
    fn test() -> Result<(), Error> {
        let object = JceObject::from([
            (0u8, JceElement::from(0)),               // Zero
            (1u8, JceElement::from(i8::MAX as i64)),  // Int8
            (2u8, JceElement::from(i16::MAX as i64)), // Int16
            (3u8, JceElement::from(i32::MAX as i64)), // Int32
            (4u8, JceElement::from(i64::MAX)),        // Int64
            (5u8, JceElement::from(1.1f32)),          // Float
            (6u8, JceElement::from(1.1f64)),          // Double
            (
                7u8,
                JceElement::from((0..255).map(|_| "a").collect::<String>()),
            ), // String1
            (
                8u8,
                JceElement::from((0..1000).map(|_| "a").collect::<String>()),
            ), // String4
            (9u8, JceElement::from([0; 128])),        // SimpleList
            (10u8, JceElement::from([JceElement::Double(2.2)])), // List
            (11u8, JceElement::from(HashMap::new())), // Empty Map
            (12u8, JceElement::from([("111", JceElement::from(1))])), // Map
            (
                250u8,
                JceElement::from(JceObject::from([(2, JceElement::from(i64::MAX))])),
            ), // Struct
            (
                251u8,
                JceElement::from(JceObject::from([(2, JceElement::from(i64::MAX))])),
            ), // Struct
        ]);
        let mut encoded = Vec::with_capacity(3000);
        encode_stream(&mut encoded, &object)?;

        let decoded = decode(&mut encoded.as_slice())?;
        // 没空写自动化测试，直接查看输出结果对不对吧
        println!("{}", format_obj(&decoded));

        Ok(())
    }

    #[test]
    fn test_wrapper() -> Result<(), Error> {
        let map = [(
            "ccc",
            JceElement::from([
                ("ddd", JceElement::from(1)),
                ("eee", JceElement::from("aaa")),
            ]), // Map
        )];
        let servant = "c";
        let func = "d";
        let req_id = 32;

        let encoded = encode_wrapper(map, servant, func, Some(req_id))?;
        let decoded = decode_wrapper(&mut encoded.as_slice())?;
        // 没空写自动化测试，直接查看输出结果对不对吧
        println!("{:?}", decoded);

        Ok(())
    }

    fn format_obj(object: &JceObject) -> String {
        let mut entries = object.iter().collect::<Vec<(&u8, &JceElement)>>();
        entries.sort_by(|a, b| (*a.0).cmp(b.0));
        let result = entries
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v.to_string()))
            .collect::<Vec<String>>()
            .join(", ");

        format!("{{ {} }}", result)
    }
}
