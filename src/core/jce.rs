use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt::Display,
    io::{Read, Write},
};

use super::{
    error::{Error, ErrorKind},
    io::WriteExt,
};

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
    use std::net::{Ipv4Addr, SocketAddrV4};

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
                    (
                        2,
                        ProtobufElement::from(current_unix_timestamp_as_secs() as isize),
                    ),
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
    fn test_push() -> Result<(), Error> {
        let buf = [
            9, 0, 1, 10, 22, 12, 49, 56, 51, 46, 53, 55, 46, 53, 51, 46, 49, 54, 33, 31, 144, 11,
            25, 0, 4, 10, 22, 12, 49, 52, 46, 49, 55, 46, 51, 50, 46, 49, 55, 51, 32, 80, 11, 10,
            22, 12, 49, 56, 51, 46, 54, 48, 46, 51, 46, 49, 51, 57, 32, 80, 11, 10, 22, 12, 49, 56,
            51, 46, 54, 48, 46, 51, 46, 50, 51, 56, 32, 80, 11, 10, 22, 12, 49, 52, 46, 49, 55, 46,
            51, 50, 46, 49, 57, 55, 32, 80, 11, 41, 0, 2, 10, 22, 12, 49, 52, 46, 50, 57, 46, 49,
            48, 49, 46, 52, 53, 32, 80, 11, 10, 22, 12, 49, 49, 51, 46, 57, 54, 46, 49, 56, 46, 55,
            55, 32, 80, 11, 57, 0, 6, 10, 22, 14, 49, 49, 57, 46, 49, 52, 55, 46, 49, 57, 46, 50,
            52, 49, 32, 80, 11, 10, 22, 14, 49, 49, 57, 46, 49, 52, 55, 46, 49, 57, 46, 50, 52, 52,
            32, 80, 11, 10, 22, 14, 49, 49, 57, 46, 49, 52, 55, 46, 49, 57, 46, 50, 52, 53, 32, 80,
            11, 10, 22, 14, 49, 49, 57, 46, 49, 52, 55, 46, 49, 57, 46, 50, 53, 50, 32, 80, 11, 10,
            22, 14, 49, 49, 57, 46, 49, 52, 55, 46, 49, 57, 46, 50, 53, 51, 32, 80, 11, 10, 22, 17,
            115, 99, 97, 110, 110, 111, 110, 46, 51, 103, 46, 113, 113, 46, 99, 111, 109, 32, 80,
            11, 73, 0, 4, 10, 22, 14, 49, 56, 51, 46, 52, 55, 46, 49, 48, 50, 46, 50, 50, 54, 32,
            80, 11, 10, 22, 10, 49, 52, 46, 50, 50, 46, 57, 46, 53, 53, 33, 31, 144, 11, 10, 22,
            13, 52, 50, 46, 56, 49, 46, 49, 56, 52, 46, 49, 56, 48, 33, 1, 187, 11, 10, 22, 15, 49,
            56, 48, 46, 49, 49, 48, 46, 49, 57, 51, 46, 49, 56, 51, 32, 80, 11, 90, 9, 0, 3, 10, 0,
            1, 25, 0, 4, 10, 0, 1, 22, 14, 49, 56, 51, 46, 52, 55, 46, 49, 48, 50, 46, 50, 50, 54,
            32, 80, 11, 10, 0, 1, 22, 10, 49, 52, 46, 50, 50, 46, 57, 46, 53, 53, 33, 31, 144, 11,
            10, 0, 1, 22, 13, 52, 50, 46, 56, 49, 46, 49, 56, 52, 46, 49, 56, 48, 33, 1, 187, 11,
            10, 0, 1, 22, 15, 49, 56, 48, 46, 49, 49, 48, 46, 49, 57, 51, 46, 49, 56, 51, 32, 80,
            11, 41, 12, 60, 11, 10, 0, 5, 25, 0, 4, 10, 0, 1, 22, 14, 49, 56, 51, 46, 52, 55, 46,
            49, 48, 50, 46, 50, 50, 54, 32, 80, 11, 10, 0, 1, 22, 10, 49, 52, 46, 50, 50, 46, 57,
            46, 53, 53, 33, 31, 144, 11, 10, 0, 1, 22, 13, 52, 50, 46, 56, 49, 46, 49, 56, 52, 46,
            49, 56, 48, 33, 1, 187, 11, 10, 0, 1, 22, 15, 49, 56, 48, 46, 49, 49, 48, 46, 49, 57,
            51, 46, 49, 56, 51, 32, 80, 11, 41, 12, 60, 11, 10, 0, 10, 25, 0, 4, 10, 0, 1, 22, 14,
            49, 56, 51, 46, 52, 55, 46, 49, 48, 50, 46, 50, 50, 54, 32, 80, 11, 10, 0, 1, 22, 10,
            49, 52, 46, 50, 50, 46, 57, 46, 53, 53, 33, 31, 144, 11, 10, 0, 1, 22, 13, 52, 50, 46,
            56, 49, 46, 49, 56, 52, 46, 49, 56, 48, 33, 1, 187, 11, 10, 0, 1, 22, 15, 49, 56, 48,
            46, 49, 49, 48, 46, 49, 57, 51, 46, 49, 56, 51, 32, 80, 11, 41, 0, 5, 10, 12, 17, 32,
            0, 32, 16, 48, 1, 11, 10, 0, 1, 17, 32, 0, 32, 8, 48, 2, 11, 10, 0, 2, 17, 32, 0, 32,
            8, 48, 1, 11, 10, 0, 3, 18, 0, 1, 0, 0, 32, 8, 48, 2, 11, 10, 0, 4, 17, 32, 0, 32, 8,
            48, 2, 11, 60, 11, 29, 0, 0, 104, 74, 209, 220, 154, 13, 198, 234, 104, 103, 145, 173,
            147, 230, 107, 39, 217, 35, 156, 208, 99, 182, 200, 225, 237, 46, 191, 171, 27, 136,
            57, 207, 51, 100, 12, 216, 144, 247, 144, 150, 199, 168, 69, 63, 70, 64, 190, 48, 200,
            214, 168, 95, 43, 200, 227, 56, 24, 15, 229, 196, 72, 100, 196, 38, 226, 224, 213, 23,
            172, 210, 50, 175, 140, 25, 159, 195, 14, 51, 157, 200, 97, 22, 135, 246, 171, 99, 77,
            126, 203, 4, 155, 8, 118, 149, 36, 252, 26, 140, 105, 115, 178, 19, 147, 109, 23, 45,
            0, 0, 16, 72, 73, 83, 57, 71, 110, 65, 113, 100, 104, 84, 68, 67, 54, 98, 67, 50, 38,
            41, 229, 184, 64, 1, 93, 0, 1, 3, 250, 138, 80, 246, 7, 10, 104, 74, 209, 220, 154, 13,
            198, 234, 104, 103, 145, 173, 147, 230, 107, 39, 217, 35, 156, 208, 99, 182, 200, 225,
            237, 46, 191, 171, 27, 136, 57, 207, 51, 100, 12, 216, 144, 247, 144, 150, 199, 168,
            69, 63, 70, 64, 190, 48, 200, 214, 168, 95, 43, 200, 227, 56, 24, 15, 229, 196, 72,
            100, 196, 38, 226, 224, 213, 23, 172, 210, 50, 175, 140, 25, 159, 195, 14, 51, 157,
            200, 97, 22, 135, 246, 171, 99, 77, 126, 203, 4, 155, 8, 118, 149, 36, 252, 26, 140,
            105, 115, 178, 19, 147, 109, 23, 18, 16, 72, 73, 83, 57, 71, 110, 65, 113, 100, 104,
            84, 68, 67, 54, 98, 67, 26, 205, 1, 8, 1, 18, 13, 8, 1, 21, 183, 47, 102, 226, 24, 80,
            32, 1, 40, 1, 18, 14, 8, 1, 21, 14, 22, 9, 55, 24, 144, 63, 32, 1, 40, 1, 18, 14, 8, 1,
            21, 42, 81, 184, 180, 24, 187, 3, 32, 2, 40, 0, 18, 13, 8, 1, 21, 180, 110, 193, 183,
            24, 80, 32, 4, 40, 0, 42, 26, 8, 1, 18, 16, 36, 14, 9, 124, 0, 47, 0, 5, 0, 0, 0, 0, 0,
            0, 0, 119, 24, 80, 32, 1, 40, 0, 42, 27, 8, 1, 18, 16, 36, 14, 9, 124, 0, 47, 0, 5, 0,
            0, 0, 0, 0, 0, 0, 113, 24, 144, 63, 32, 1, 40, 0, 42, 26, 8, 1, 18, 16, 36, 14, 9, 40,
            20, 0, 1, 5, 0, 0, 0, 0, 0, 0, 0, 64, 24, 80, 32, 2, 40, 0, 42, 26, 8, 1, 18, 16, 36,
            14, 0, 233, 96, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 7, 24, 80, 32, 4, 40, 0, 42, 26, 8, 1,
            18, 16, 36, 8, 135, 86, 15, 80, 0, 5, 0, 0, 0, 0, 0, 0, 0, 48, 24, 80, 32, 1, 40, 0,
            26, 205, 1, 8, 5, 18, 13, 8, 1, 21, 183, 47, 102, 226, 24, 80, 32, 1, 40, 1, 18, 14, 8,
            1, 21, 14, 22, 9, 55, 24, 144, 63, 32, 1, 40, 1, 18, 14, 8, 1, 21, 42, 81, 184, 180,
            24, 187, 3, 32, 2, 40, 0, 18, 13, 8, 1, 21, 180, 110, 193, 183, 24, 80, 32, 4, 40, 0,
            42, 26, 8, 1, 18, 16, 36, 14, 9, 124, 0, 47, 0, 5, 0, 0, 0, 0, 0, 0, 0, 119, 24, 80,
            32, 1, 40, 0, 42, 27, 8, 1, 18, 16, 36, 14, 9, 124, 0, 47, 0, 5, 0, 0, 0, 0, 0, 0, 0,
            113, 24, 144, 63, 32, 1, 40, 0, 42, 26, 8, 1, 18, 16, 36, 14, 9, 40, 20, 0, 1, 5, 0, 0,
            0, 0, 0, 0, 0, 64, 24, 80, 32, 2, 40, 0, 42, 26, 8, 1, 18, 16, 36, 14, 0, 233, 96, 3,
            2, 0, 0, 0, 0, 0, 0, 0, 0, 7, 24, 80, 32, 4, 40, 0, 42, 26, 8, 1, 18, 16, 36, 8, 135,
            86, 15, 80, 0, 5, 0, 0, 0, 0, 0, 0, 0, 48, 24, 80, 32, 1, 40, 0, 26, 133, 2, 8, 10, 18,
            13, 8, 1, 21, 183, 47, 102, 226, 24, 80, 32, 1, 40, 1, 18, 14, 8, 1, 21, 14, 22, 9, 55,
            24, 144, 63, 32, 1, 40, 1, 18, 14, 8, 1, 21, 42, 81, 184, 180, 24, 187, 3, 32, 2, 40,
            0, 18, 13, 8, 1, 21, 180, 110, 193, 183, 24, 80, 32, 4, 40, 0, 34, 9, 8, 0, 16, 128,
            64, 24, 16, 32, 1, 34, 9, 8, 1, 16, 128, 64, 24, 8, 32, 2, 34, 9, 8, 2, 16, 128, 64,
            24, 8, 32, 1, 34, 10, 8, 3, 16, 128, 128, 4, 24, 8, 32, 2, 34, 9, 8, 4, 16, 128, 64,
            24, 8, 32, 2, 42, 26, 8, 1, 18, 16, 36, 14, 9, 124, 0, 47, 0, 5, 0, 0, 0, 0, 0, 0, 0,
            119, 24, 80, 32, 1, 40, 0, 42, 27, 8, 1, 18, 16, 36, 14, 9, 124, 0, 47, 0, 5, 0, 0, 0,
            0, 0, 0, 0, 113, 24, 144, 63, 32, 1, 40, 0, 42, 26, 8, 1, 18, 16, 36, 14, 9, 40, 20, 0,
            1, 5, 0, 0, 0, 0, 0, 0, 0, 64, 24, 80, 32, 2, 40, 0, 42, 26, 8, 1, 18, 16, 36, 14, 0,
            233, 96, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 7, 24, 80, 32, 4, 40, 0, 42, 26, 8, 1, 18, 16,
            36, 8, 135, 86, 15, 80, 0, 5, 0, 0, 0, 0, 0, 0, 0, 48, 24, 80, 32, 1, 40, 0, 32, 1, 50,
            4, 8, 0, 16, 1, 58, 42, 8, 16, 16, 16, 24, 9, 32, 9, 40, 15, 48, 15, 56, 5, 64, 5, 72,
            90, 80, 1, 88, 90, 96, 90, 104, 90, 112, 90, 120, 10, 128, 1, 10, 136, 1, 10, 144, 1,
            10, 152, 1, 10, 66, 10, 8, 0, 16, 0, 24, 4, 32, 4, 40, 4, 74, 6, 8, 1, 16, 1, 24, 3,
            82, 66, 8, 2, 18, 10, 8, 0, 16, 128, 128, 4, 24, 16, 32, 2, 18, 10, 8, 1, 16, 128, 128,
            4, 24, 8, 32, 2, 18, 10, 8, 2, 16, 128, 128, 1, 24, 8, 32, 1, 18, 10, 8, 3, 16, 128,
            128, 4, 24, 8, 32, 2, 18, 10, 8, 4, 16, 128, 128, 4, 24, 8, 32, 2, 24, 1, 32, 0, 90,
            61, 8, 2, 18, 10, 8, 0, 16, 128, 128, 4, 24, 16, 32, 2, 18, 9, 8, 1, 16, 128, 64, 24,
            8, 32, 2, 18, 9, 8, 2, 16, 128, 64, 24, 8, 32, 1, 18, 10, 8, 3, 16, 128, 128, 4, 24, 8,
            32, 2, 18, 9, 8, 4, 16, 128, 64, 24, 8, 32, 2, 24, 1, 112, 2, 120, 1, 128, 1, 0, 11,
            105, 0, 1, 10, 22, 45, 105, 109, 103, 99, 97, 99, 104, 101, 46, 113, 113, 46, 99, 111,
            109, 46, 115, 99, 104, 101, 100, 46, 108, 101, 103, 111, 112, 105, 99, 49, 45, 100,
            107, 46, 116, 100, 110, 115, 118, 54, 46, 99, 111, 109, 46, 32, 80, 11, 121, 0, 2, 10,
            22, 13, 49, 52, 46, 50, 57, 46, 49, 48, 48, 46, 49, 53, 53, 32, 80, 11, 10, 22, 12, 49,
            52, 46, 50, 57, 46, 49, 48, 49, 46, 52, 56, 32, 80, 11, 138, 6, 14, 49, 49, 51, 46, 49,
            48, 57, 46, 50, 49, 56, 46, 51, 57, 16, 3, 11, 154, 9, 0, 7, 10, 0, 4, 25, 0, 1, 10,
            18, 11, 39, 89, 101, 32, 80, 11, 41, 12, 11, 10, 0, 7, 25, 0, 1, 10, 18, 79, 167, 151,
            61, 32, 80, 11, 41, 12, 11, 10, 0, 9, 25, 0, 2, 10, 18, 161, 106, 47, 183, 32, 80, 11,
            10, 18, 115, 115, 47, 183, 32, 80, 11, 41, 12, 11, 10, 0, 10, 25, 0, 2, 10, 18, 162,
            106, 47, 183, 32, 80, 11, 10, 18, 49, 115, 47, 183, 32, 80, 11, 41, 12, 11, 10, 0, 5,
            25, 0, 1, 10, 18, 29, 226, 3, 183, 32, 80, 11, 41, 12, 11, 10, 0, 8, 25, 0, 2, 10, 18,
            18, 200, 18, 14, 32, 80, 11, 10, 18, 115, 200, 18, 14, 32, 80, 11, 41, 12, 11, 10, 0,
            6, 25, 0, 2, 10, 18, 245, 246, 101, 180, 32, 80, 11, 10, 18, 164, 246, 101, 180, 32,
            80, 11, 41, 12, 11, 11, 173, 0, 1, 1, 90, 8, 1, 16, 223, 217, 180, 142, 5, 24, 0, 34,
            9, 54, 52, 48, 50, 55, 57, 57, 57, 50, 40, 241, 218, 233, 190, 2, 50, 18, 8, 183, 249,
            152, 135, 4, 16, 80, 24, 137, 144, 234, 202, 8, 32, 80, 40, 100, 50, 17, 8, 183, 223,
            180, 163, 4, 16, 80, 24, 137, 144, 130, 32, 32, 80, 40, 100, 50, 19, 8, 177, 172, 169,
            177, 1, 16, 80, 24, 139, 156, 226, 162, 2, 32, 80, 40, 200, 1, 50, 19, 8, 177, 172,
            169, 201, 5, 16, 80, 24, 139, 232, 146, 171, 15, 32, 80, 40, 200, 1, 50, 19, 8, 189,
            226, 203, 161, 10, 16, 80, 24, 137, 216, 172, 152, 15, 32, 80, 40, 172, 2, 50, 19, 8,
            189, 226, 151, 201, 5, 16, 80, 24, 137, 202, 188, 184, 9, 32, 80, 40, 172, 2, 58, 30,
            10, 16, 36, 14, 9, 124, 0, 47, 48, 2, 0, 0, 0, 0, 0, 0, 0, 21, 16, 80, 24, 139, 246,
            198, 156, 3, 32, 80, 40, 100, 58, 30, 10, 16, 36, 14, 0, 255, 241, 0, 0, 19, 0, 0, 0,
            0, 0, 0, 0, 7, 16, 80, 24, 137, 214, 227, 140, 5, 32, 80, 40, 100, 58, 31, 10, 16, 36,
            14, 0, 233, 96, 3, 2, 17, 0, 0, 0, 0, 0, 0, 0, 3, 16, 80, 24, 139, 232, 146, 171, 15,
            32, 80, 40, 200, 1, 58, 31, 10, 16, 36, 14, 0, 225, 170, 0, 0, 19, 0, 0, 0, 0, 0, 0, 0,
            183, 16, 80, 24, 137, 228, 189, 192, 14, 32, 80, 40, 200, 1, 58, 31, 10, 16, 36, 2, 78,
            0, 16, 32, 16, 111, 0, 0, 151, 121, 84, 90, 25, 45, 16, 80, 24, 137, 144, 230, 218, 15,
            32, 80, 40, 172, 2, 58, 31, 10, 16, 36, 2, 78, 0, 16, 32, 16, 111, 0, 0, 151, 121, 84,
            80, 238, 224, 16, 80, 24, 137, 144, 230, 202, 15, 32, 80, 40, 172, 2,
        ];

        let mut d = jce::decode(&mut buf.as_slice())?;
        let e = d.try_remove(&5)?;
        if let JceElement::StructBegin(mut s) = e {
            let e = s.try_remove(&5)?;

            if let JceElement::SimpleList(buf) = e {
                let mut decoded = protobuf::decode(&mut buf.as_slice())?;
                let d1281: Vec<u8> = decoded.try_remove(&1281)?.try_into()?;
                let mut d1281_decoded = protobuf::decode(&mut d1281.as_slice())?;
                let n1: Vec<u8> = d1281_decoded.try_remove(&1)?.try_into()?;
                let n2: Vec<u8> = d1281_decoded.try_remove(&2)?.try_into()?;
                let n3: Vec<ProtobufElement> = d1281_decoded.try_remove(&3)?.try_into()?;

                for v in n3 {
                    let mut v: ProtobufObject = v.try_decode_bytes()?;

                    let i: isize = v.try_remove(&1)?.try_into()?;
                    if i == 10 {
                        let mut n3_list: Vec<ProtobufElement> = v.try_remove(&2)?.try_into()?;
                        let mut n3_decoded: ProtobufObject =
                            n3_list.remove(0).try_decode_bytes()?;

                        let ip_integer: isize = n3_decoded.try_remove(&2)?.try_into()?;
                        let ipv4 = Ipv4Addr::from(ip_integer as u32);
                        let port: isize = n3_decoded.try_remove(&3)?.try_into()?;
                        let socketAddr = SocketAddrV4::new(ipv4, port as u16);

                        println!("{}", socketAddr);
                    }
                }
            }
        };

        Ok(())
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
