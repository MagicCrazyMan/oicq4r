use std::collections::HashMap;

use protobuf::CodedOutputStream;

/// 标记可以编码为 Protobuf 的数据
pub trait EncodeProtobuf {
    fn size_hint(&self) -> usize;

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error>;
}

impl EncodeProtobuf for Vec<u8> {
    fn size_hint(&self) -> usize {
        self.len()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        self.as_slice().encode_protobuf(tag, stream)
    }
}

impl<const N: usize> EncodeProtobuf for [u8; N] {
    fn size_hint(&self) -> usize {
        self.len()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        self.as_slice().encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for &[u8] {
    fn size_hint(&self) -> usize {
        self.len()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        stream.write_bytes(tag, self)
    }
}

impl EncodeProtobuf for isize {
    fn size_hint(&self) -> usize {
        (isize::BITS / 8) as usize
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for i64 {
    fn size_hint(&self) -> usize {
        8
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        if *self < 0 {
            stream.write_sint64(tag, *self)
        } else {
            stream.write_int64(tag, *self)
        }
    }
}

impl EncodeProtobuf for i32 {
    fn size_hint(&self) -> usize {
        4
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for i16 {
    fn size_hint(&self) -> usize {
        2
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for i8 {
    fn size_hint(&self) -> usize {
        1
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for usize {
    fn size_hint(&self) -> usize {
        (usize::BITS / 8) as usize
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for u64 {
    fn size_hint(&self) -> usize {
        8
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for u32 {
    fn size_hint(&self) -> usize {
        4
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for u16 {
    fn size_hint(&self) -> usize {
        2
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for u8 {
    fn size_hint(&self) -> usize {
        1
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as i64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for f64 {
    fn size_hint(&self) -> usize {
        8
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        stream.write_double(tag, *self)
    }
}

impl EncodeProtobuf for f32 {
    fn size_hint(&self) -> usize {
        4
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        (*self as f64).encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for &str {
    fn size_hint(&self) -> usize {
        self.as_bytes().len()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        stream.write_string(tag, self)
    }
}

impl<'a, T> EncodeProtobuf for T
where
    T: AsRef<EncodedObject<'a>>,
{
    fn size_hint(&self) -> usize {
        let obj = self.as_ref();
        obj.len() * 4 + obj.iter().map(|(_, e)| e.size_hint()).sum::<usize>()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        stream.write_bytes(tag, &self.as_ref().encode()?)
    }
}

impl EncodeProtobuf for Vec<&dyn EncodeProtobuf> {
    fn size_hint(&self) -> usize {
        self.as_slice().size_hint()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        self.as_slice().encode_protobuf(tag, stream)
    }
}

impl<const N: usize> EncodeProtobuf for [&dyn EncodeProtobuf; N] {
    fn size_hint(&self) -> usize {
        self.as_slice().size_hint()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        self.as_slice().encode_protobuf(tag, stream)
    }
}

impl EncodeProtobuf for &[&dyn EncodeProtobuf] {
    fn size_hint(&self) -> usize {
        self.iter().map(|e| e.size_hint()).sum()
    }

    fn encode_protobuf(
        &self,
        tag: u32,
        stream: &mut CodedOutputStream,
    ) -> Result<(), protobuf::Error> {
        self.iter().try_for_each(|e| e.encode_protobuf(tag, stream))
    }
}

/// Protobuf Object 容器
pub struct EncodedObject<'a>(HashMap<u32, &'a dyn EncodeProtobuf>);

impl<'a> EncodedObject<'a> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    /// debug 模式下，HashMap 会按照 key(u8) 排序后生成 JceObject，以方便配合 nodejs debug
    #[cfg(debug_assertions)]
    pub fn encode(&self) -> Result<Vec<u8>, protobuf::Error> {
        // 这个 length_hint 不保证计算的大小完全符合实际编码后的大小，仅保证贴近编码后的大小，以避免多次申请内存空间或申请过多内存空间
        let length_hint =
            self.0.len() * 4 + self.0.iter().map(|(_, e)| e.size_hint()).sum::<usize>();

        let mut buf = Vec::with_capacity(length_hint);
        let mut stream = CodedOutputStream::new(&mut buf);

        let mut list = self.iter().collect::<Vec<_>>();
        list.sort_by(|a, b| (*a.0).cmp(b.0));

        list.into_iter()
            .try_for_each(|(tag, element)| element.encode_protobuf(*tag, &mut stream))?;

        drop(stream);
        Ok(buf)
    }

    #[cfg(not(debug_assertions))]
    fn encode(&self) -> Result<Vec<u8>, DecodeProtobufError> {
        let mut buf = Vec::with_capacity(1024);
        let mut stream = CodedOutputStream::new(&mut buf);

        self.iter()
            .try_for_each(|(tag, element)| element.encode_protobuf(*tag, &mut stream))?;

        drop(stream);
        Ok(buf)
    }
}

impl<'a, const N: usize> From<[(u32, &'a dyn EncodeProtobuf); N]> for EncodedObject<'a> {
    fn from(arr: [(u32, &'a dyn EncodeProtobuf); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

impl<'a, const N: usize> From<[&'a dyn EncodeProtobuf; N]> for EncodedObject<'a> {
    fn from(arr: [&'a dyn EncodeProtobuf; N]) -> Self {
        Self(
            arr.into_iter()
                .enumerate()
                .map(|(i, e)| (i as u32, e))
                .collect::<HashMap<_, _>>(),
        )
    }
}

impl<'a> std::ops::DerefMut for EncodedObject<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> std::ops::Deref for EncodedObject<'a> {
    type Target = HashMap<u32, &'a dyn EncodeProtobuf>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> AsRef<EncodedObject<'a>> for EncodedObject<'a> {
    fn as_ref(&self) -> &EncodedObject<'a> {
        self
    }
}

#[macro_export]
macro_rules! to_protobuf {
    ($t:expr) => {
        (&$t as &dyn EncodeProtobuf)
    };
}
