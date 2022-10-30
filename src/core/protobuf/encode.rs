use std::collections::HashMap;

use protobuf::CodedOutputStream;

/// Protobuf 编码元素
///
/// 所持有的所有实际元素均为引用或基础数据类型，元素容器及对象容器则持有所有权。
/// 在保证代码可读性的同时避免重复克隆实际数据
#[derive(Debug, Clone)]
pub enum Element<'a> {
    Integer(i128),
    Double(f64),
    Bytes(&'a [u8]),
    String(&'a str),
    Array(Vec<Element<'a>>),
    Object(Object<'a>),
}

impl<'a> Element<'a> {
    fn size_hint(&self) -> usize {
        match self {
            Element::Integer(_) => 8,
            Element::Double(_) => 8,
            Element::Bytes(v) => v.len(),
            Element::String(v) => v.as_bytes().len(),
            Element::Array(a) => a.iter().map(|v| v.size_hint()).sum(),
            Element::Object(o) => o.len() * 4 + o.iter().map(|(_, v)| v.size_hint()).sum::<usize>(),
        }
    }

    fn encode(&self, tag: u32, stream: &mut CodedOutputStream) -> Result<(), protobuf::Error> {
        match self {
            Element::Integer(v) => {
                let v = *v;
                if v < 0 {
                    stream.write_sint64(tag, v as i64)
                } else {
                    stream.write_int64(tag, v as i64)
                }
            }
            Element::Double(v) => stream.write_double(tag, *v),
            Element::Bytes(b) => stream.write_bytes(tag, *b),
            Element::String(s) => stream.write_string(tag, *s),
            Element::Array(list) => list.iter().try_for_each(|e| e.encode(tag, stream)),
            Element::Object(obj) => stream.write_bytes(tag, obj.encode()?.as_slice()),
        }
    }
}

impl<'a> From<isize> for Element<'a> {
    fn from(value: isize) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<usize> for Element<'a> {
    fn from(value: usize) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<i128> for Element<'a> {
    fn from(value: i128) -> Self {
        Self::Integer(value)
    }
}

impl<'a> From<u128> for Element<'a> {
    fn from(value: u128) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<i64> for Element<'a> {
    fn from(value: i64) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<u64> for Element<'a> {
    fn from(value: u64) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<i32> for Element<'a> {
    fn from(value: i32) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<u32> for Element<'a> {
    fn from(value: u32) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<i16> for Element<'a> {
    fn from(value: i16) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<u16> for Element<'a> {
    fn from(value: u16) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<i8> for Element<'a> {
    fn from(value: i8) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<u8> for Element<'a> {
    fn from(value: u8) -> Self {
        Self::Integer(value as i128)
    }
}

impl<'a> From<f32> for Element<'a> {
    fn from(value: f32) -> Self {
        Self::Double(value as f64)
    }
}

impl<'a> From<f64> for Element<'a> {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl<'a> From<&'a str> for Element<'a> {
    fn from(value: &'a str) -> Self {
        Self::String(value)
    }
}

impl<'a> From<&'a [u8]> for Element<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::Bytes(value)
    }
}

// impl<'a> From<&'a [Element<'a>]> for Element<'a> {
//     fn from(value: &'a [Element<'a>]) -> Self {
//         Self::Array(value.to_vec())
//     }
// }

impl<'a, const N: usize> From<[Element<'a>; N]> for Element<'a> {
    fn from(value: [Element<'a>; N]) -> Self {
        Self::Array(value.to_vec())
    }
}

impl<'a> From<Vec<Element<'a>>> for Element<'a> {
    fn from(value: Vec<Element<'a>>) -> Self {
        Self::Array(value)
    }
}

impl<'a> From<Object<'a>> for Element<'a> {
    fn from(value: Object<'a>) -> Self {
        Self::Object(value)
    }
}

/// Protobuf Object 容器
#[derive(Debug, Clone)]
pub struct Object<'a>(HashMap<u32, Element<'a>>);

impl<'a> std::ops::DerefMut for Object<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> std::ops::Deref for Object<'a> {
    type Target = HashMap<u32, Element<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> Object<'a> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    fn prepare_decode_buf(&self) -> Vec<u8> {
        // 这个 length_hint 不保证计算的大小完全符合实际编码后的大小，仅保证贴近编码后的大小，以避免多次申请内存空间或申请过多内存空间
        let length_hint =
            self.0.len() * 4 + self.0.iter().map(|(_, e)| e.size_hint()).sum::<usize>();
        Vec::with_capacity(length_hint)
    }

    /// debug 模式下，HashMap 会按照 key(u32) 排序后生成 Object，以方便配合 nodejs debug
    #[cfg(debug_assertions)]
    pub fn encode(&self) -> Result<Vec<u8>, protobuf::Error> {
        let mut buf = self.prepare_decode_buf();
        let mut stream = CodedOutputStream::new(&mut buf);

        let mut list = self.iter().collect::<Vec<_>>();
        list.sort_by(|a, b| (*a.0).cmp(b.0));

        list.into_iter()
            .try_for_each(|(tag, element)| element.encode(*tag, &mut stream))?;

        drop(stream);
        Ok(buf)
    }

    #[cfg(not(debug_assertions))]
    fn encode(&self) -> Result<Vec<u8>, DecodeProtobufError> {
        let mut buf = self.prepare_decode_buf();
        let mut stream = CodedOutputStream::new(&mut buf);

        self.iter()
            .try_for_each(|(tag, element)| element.encode(*tag, &mut stream))?;

        drop(stream);
        Ok(buf)
    }
}

impl<'a, const N: usize> From<[(u32, Element<'a>); N]> for Object<'a> {
    fn from(arr: [(u32, Element<'a>); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

impl<'a, const N: usize> From<[Element<'a>; N]> for Object<'a> {
    fn from(arr: [Element<'a>; N]) -> Self {
        Self(
            arr.into_iter()
                .enumerate()
                .map(|(i, e)| (i as u32, e))
                .collect::<HashMap<_, _>>(),
        )
    }
}
