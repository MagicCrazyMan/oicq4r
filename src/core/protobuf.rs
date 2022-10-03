use std::{
    collections::HashMap,
    io::{Read, Write},
};

use protobuf::{rt::WireType, CodedInputStream, CodedOutputStream};

use super::error::CommonError;

#[derive(Debug, Clone)]
pub enum ProtobufElement {
    Null,
    Integer(i64),
    Double(f64),
    Bytes(Vec<u8>),
    String(String),
    Array(Vec<ProtobufElement>),
    Object(ProtobufObject),
}

impl From<i64> for ProtobufElement {
    fn from(value: i64) -> Self {
        Self::Integer(value)
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
) -> Result<(), CommonError> {
    match element {
        ProtobufElement::Null => {}
        ProtobufElement::Integer(value) => {
            if *value < 0 {
                writer.write_sint64(tag, *value)?;
            } else {
                writer.write_int64(tag, *value)?;
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
fn encode_object(writer: &mut CodedOutputStream, object: &ProtobufObject) -> Result<(), CommonError> {
    let mut list = object.iter().collect::<Vec<_>>();
    list.sort_by(|a, b| (*a.0).cmp(b.0));

    list.into_iter()
        .try_for_each(|(tag, element)| encode_element(writer, *tag, element))
}

#[cfg(not(debug_assertions))]
fn encode_object(writer: &mut CodedOutputStream, object: &ProtobufObject) -> Result<(), CommonError> {
    object
        .iter()
        .try_for_each(|(tag, element)| encode_element(writer, *tag, element))
}

pub fn encode_stream<W>(writer: &mut W, object: &ProtobufObject) -> Result<(), CommonError>
where
    W: Write,
{
    let mut coded_stream = CodedOutputStream::new(writer);
    encode_object(&mut coded_stream, object)
}

pub fn encode_with_capacity(object: &ProtobufObject, capacity: usize) -> Result<Vec<u8>, CommonError> {
    let mut buf = Vec::with_capacity(capacity);
    let mut coded_stream = CodedOutputStream::new(&mut buf);
    encode_object(&mut coded_stream, object)?;
    drop(coded_stream);

    Ok(buf)
}

pub fn encode(object: &ProtobufObject) -> Result<Vec<u8>, CommonError> {
    encode_with_capacity(object, 1000)
}

// fn decode_element(&mut reader: CodedInputStream) -> Result<(), Error> {

// }

// pub fn decode_stream(reader: &mut CodedInputStream) -> Result<ProtobufObject, Error> {
//     let mut result = ProtobufObject::with_capacity(20);

//     while !reader.eof()? {
//         let k = reader.read_uint32()?;
//         let tag = k >> 3;
//         let r#type = k & 0b111;

//         let value = match r#type {
//             0 => ProtobufElement::I64(reader.read_int64()?),
//             1 => ProtobufElement::Double(reader.read_double()?),
//             2 => ProtobufElement::Double(reader.read_bytes()?),
//             5 => ProtobufElement::Double(reader.read_fixed32()?),
//             _ => return Err(Error::from("Invalid protobuf type")),
//         };

//         if let Some(e) = result.remove(&tag) {
//             let v = if let ProtobufElement::Array(mut list) = e {
//                 list.push(value);
//                 e
//             } else {
//                 ProtobufElement::Array(vec![e])
//             };

//             result.insert(tag, v);
//         } else {
//             result.insert(tag, value);
//         }
//     }

//     Ok(result)
// }

// pub fn decode<R: Read>(reader: &mut R) -> Result<ProtobufObject, Error> {
//     let mut reader = CodedInputStream::new(reader);
//     decode_stream(&mut reader)
// }

#[cfg(test)]
mod test {
    use crate::core::{
        error::CommonError,
        writer::{write_bytes, write_u32},
    };

    use super::{encode, ProtobufElement, ProtobufObject};

    #[test]
    fn test() -> Result<(), CommonError> {
        let mut buf = Vec::with_capacity(9);
        write_u32(&mut buf, 324372432)?;
        write_bytes(&mut buf, [0x00, 0x00, 0x01, 0x9e, 0x39])?;

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
