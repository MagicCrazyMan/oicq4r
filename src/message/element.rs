use std::{
    fs::File,
    io::{Cursor, Read},
    path::PathBuf,
    time::Duration,
};

use reqwest::{header::HeaderMap, IntoUrl};
use serde_json::Value;

use crate::error::Error;

pub trait MessageElement {}
pub trait ChainElement {}

/// @某人或全员
#[derive(Debug, Clone)]
pub struct TextElement {
    pub typee: &'static str,
    pub text: String,
}

impl TextElement {
    pub fn new(text: String) -> Self {
        Self {
            typee: "text",
            text,
        }
    }
}

impl MessageElement for TextElement {}
impl ChainElement for TextElement {}

#[derive(Debug, Clone)]
pub enum ATTarget {
    /// @ 指定用户
    User(u32),
    /// @ 频道
    Channel(u32),
    /// @ 全体
    All,
}

/// @某人或全员
#[derive(Debug, Clone)]
pub struct ATElement {
    pub typee: &'static str,
    /// 在频道信息中该值为 0
    pub qq: ATTarget,
    pub text: Option<String>,
    /// 是否为假 AT
    pub dummy: bool,
}

impl ATElement {
    pub fn new(target: ATTarget, text: Option<String>, dummy: bool) -> Self {
        Self {
            typee: "at",
            qq: target,
            text,
            dummy,
        }
    }
}

impl MessageElement for ATElement {}
impl ChainElement for ATElement {}

/// 表情
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceType {
    Face,
    SFace,
}

impl AsRef<str> for FaceType {
    fn as_ref(&self) -> &str {
        match self {
            FaceType::Face => "face",
            FaceType::SFace => "sface",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FaceElement {
    pub typee: FaceType,
    /// face为 0 ~ 324，sface 不明
    pub id: usize,
    pub text: Option<String>,
}

impl FaceElement {
    pub fn new(typee: FaceType, id: usize, text: Option<String>) -> Self {
        Self { typee, id, text }
    }
}

impl MessageElement for FaceElement {}
impl ChainElement for FaceElement {}

/// 原创表情
#[derive(Debug, Clone)]
pub struct BFaceElement {
    pub typee: &'static str,
    /// 暂时只能发收到的file
    pub file: String,
    pub text: String,
}

impl BFaceElement {
    pub fn new(file: String, text: String) -> Self {
        Self {
            typee: "bface",
            file,
            text,
        }
    }
}

impl MessageElement for BFaceElement {}
impl ChainElement for BFaceElement {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MFaceType {
    RPS,
    DICE,
}

impl AsRef<str> for MFaceType {
    fn as_ref(&self) -> &str {
        match self {
            MFaceType::RPS => "rps",
            MFaceType::DICE => "dice",
        }
    }
}

/// 魔法表情
#[derive(Debug, Clone)]
pub struct MFaceElement {
    pub typee: MFaceType,
    pub id: Option<usize>,
}

impl MFaceElement {
    pub fn new(typee: MFaceType, id: Option<usize>) -> Self {
        Self { typee, id }
    }
}

impl MessageElement for MFaceElement {}
impl ChainElement for MFaceElement {}

/// 图片
#[derive(Debug, Clone)]
pub struct ImageElement {
    pub typee: &'static str,
    pub data: Vec<u8>,
    /// 图片类型,有下列几种
    /// - jpg: `1000`
    /// - png: `1001`
    /// - webp: `1002`
    /// - bmp: `1005`
    /// - gif: `2000`
    /// - face: `4` (该类型仅允许从文件名中读取)
    /// - 无法解析或不存在将视为 jpg
    pub format: u32,
    /// 图片大小，此处会直接使用 data 的长度
    pub size: u64,
    /// 图片宽度
    pub width: u32,
    /// 图片高度
    pub height: u32,
    /// data 的 md5 哈希结果
    pub md5: [u8; 16],
    /// 网络图片是否使用缓存
    pub cache: bool,
    /// 是否作为表情发送
    pub as_face: bool,
    /// 是否显示下载原图按钮
    pub origin: bool,
}

impl ImageElement {
    pub fn new(
        data: Vec<u8>,
        force_format: Option<u32>,
        cache: Option<bool>,
        as_face: Option<bool>,
        origin: Option<bool>,
    ) -> Result<Self, Error> {
        let md5 = md5::compute(&data).0;

        let reader = image::io::Reader::new(Cursor::new(&data)).with_guessed_format()?;
        let format = if let Some(format) = force_format {
            format
        } else {
            let format = reader.format();

            if let Some(format) = format {
                match format {
                    image::ImageFormat::Png => 1001,
                    image::ImageFormat::Jpeg => 1000,
                    image::ImageFormat::Gif => 2000,
                    image::ImageFormat::WebP => 1002,
                    image::ImageFormat::Bmp => 1005,
                    _ => 1000,
                }
            } else {
                1000
            }
        };
        let (width, height) = reader.into_dimensions()?;

        Ok(Self {
            typee: "image",
            format,
            size: data.len() as u64,
            data,
            md5,
            width,
            height,
            cache: cache.unwrap_or(false),
            as_face: as_face.unwrap_or(false),
            origin: origin.unwrap_or(false),
        })
    }

    pub async fn from_file(
        path: PathBuf,
        cache: Option<bool>,
        as_face: Option<bool>,
        origin: Option<bool>,
    ) -> Result<Self, Error> {
        let typee = path.extension().and_then(|ext| ext.eq("face").then_some(4));

        let mut file = tokio::fs::OpenOptions::new().read(true).open(path).await?;
        let metadata = file.metadata().await?;
        let mut buf = Vec::with_capacity(metadata.len() as usize);

        tokio::io::AsyncReadExt::read_to_end(&mut file, &mut buf).await?;

        Self::new(buf, typee, cache, as_face, origin)
    }

    /// http:// 或 https://
    pub async fn from_web<U>(
        url: U,
        timeout: Option<Duration>,
        headers: Option<HeaderMap>,
        cache: Option<bool>,
        as_face: Option<bool>,
        origin: Option<bool>,
    ) -> Result<Self, Error>
    where
        U: IntoUrl,
    {
        let client = reqwest::Client::new();
        let mut builder = client.get(url);
        if let Some(headers) = headers {
            builder = builder.headers(headers);
        }
        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }

        let response = builder.send().await?;
        let data = response.bytes().await?.to_vec();

        Self::new(data, None, cache, as_face, origin)
    }

    /// base64://
    pub async fn from_base64<S>(
        base64_data: S,
        cache: Option<bool>,
        as_face: Option<bool>,
        origin: Option<bool>,
    ) -> Result<Self, Error>
    where
        S: AsRef<str>,
    {
        let base64_data = base64_data.as_ref();
        let data = base64::decode(&base64_data[9..])?;

        Self::new(data, None, cache, as_face, origin)
    }
}

impl MessageElement for ImageElement {}
impl ChainElement for ImageElement {}

/// 闪照
#[derive(Debug, Clone)]
pub struct FlashImageElement {
    pub typee: &'static str,
    pub image: ImageElement,
}

impl FlashImageElement {
    pub fn new(image: ImageElement) -> Self {
        Self {
            typee: "flash",
            image,
        }
    }
}

impl MessageElement for FlashImageElement {}

/// 语音
#[derive(Debug, Clone)]
pub struct PTTElement {
    pub typee: &'static str,
    pub file: Vec<u8>,
    pub url: Option<String>,
    pub md5: Option<String>,
    pub size: Option<usize>,
    pub seconds: Option<usize>,
}

impl PTTElement {
    pub fn new(
        file: Vec<u8>,
        url: Option<String>,
        md5: Option<String>,
        size: Option<usize>,
        seconds: Option<usize>,
    ) -> Self {
        Self {
            typee: "record",
            file,
            url,
            md5,
            size,
            seconds,
        }
    }
}

impl MessageElement for PTTElement {}

/// 视频
#[derive(Debug, Clone)]
pub struct VideoElement {
    pub typee: &'static str,
    pub file: String,
    pub name: Option<String>,
    pub fid: Option<String>,
    pub md5: Option<String>,
    pub size: Option<usize>,
    pub seconds: Option<usize>,
}

impl VideoElement {
    pub fn new(
        file: String,
        name: Option<String>,
        fid: Option<String>,
        md5: Option<String>,
        size: Option<usize>,
        seconds: Option<usize>,
    ) -> Self {
        Self {
            typee: "video",
            file,
            name,
            fid,
            md5,
            size,
            seconds,
        }
    }
}

impl MessageElement for VideoElement {}

/// 位置分享
#[derive(Debug, Clone)]
pub struct LocationElement {
    pub typee: &'static str,
    pub address: String,
    pub lat: f64,
    pub lng: f64,
    pub name: Option<String>,
    pub id: Option<String>,
}

impl LocationElement {
    pub fn new(
        address: String,
        lat: f64,
        lng: f64,
        name: Option<String>,
        id: Option<String>,
    ) -> Self {
        Self {
            typee: "location",
            address,
            lat,
            lng,
            name,
            id,
        }
    }
}

impl MessageElement for LocationElement {}

/// 分享链接
#[derive(Debug, Clone)]
pub struct ShareElement {
    pub typee: &'static str,
    pub url: String,
    pub title: String,
    pub content: Option<String>,
    pub image: Option<String>,
}

impl ShareElement {
    pub fn new(url: String, title: String, content: Option<String>, image: Option<String>) -> Self {
        Self {
            typee: "share",
            url,
            title,
            content,
            image,
        }
    }
}

impl MessageElement for ShareElement {}

/// JSON 数据
#[derive(Debug, Clone)]
pub struct JSONElement {
    pub typee: &'static str,
    pub data: Value,
}

impl JSONElement {
    pub fn new(data: Value) -> Self {
        Self {
            typee: "json",
            data,
        }
    }
}

impl MessageElement for JSONElement {}

/// XML 数据
#[derive(Debug, Clone)]
pub struct XMLElement {
    pub typee: &'static str,
    pub data: String,
    pub id: Option<usize>,
}

impl XMLElement {
    pub fn new(data: String, id: Option<usize>) -> Self {
        Self {
            typee: "xml",
            data,
            id,
        }
    }
}

impl MessageElement for XMLElement {}

/// 戳一戳
#[derive(Debug, Clone)]
pub struct PokeElement {
    pub typee: &'static str,
    pub id: usize,
    pub text: Option<String>,
}

impl PokeElement {
    pub fn new(id: usize, text: Option<String>) -> Self {
        Self {
            typee: "poke",
            id,
            text,
        }
    }
}

impl MessageElement for PokeElement {}

/// 特殊消息，官方客户端暂时无法解析此消息
#[derive(Debug, Clone)]
pub struct MiraiElement {
    pub typee: &'static str,
    pub id: String,
}

impl MiraiElement {
    pub fn new(id: String) -> Self {
        Self { typee: "mirai", id }
    }
}

impl MessageElement for MiraiElement {}
impl ChainElement for MiraiElement {}

/// 文件，暂时只支持接收，发送需要使用文件专用 API
#[derive(Debug, Clone)]
pub struct FileElement {
    pub typee: &'static str,
    pub name: String,
    pub fid: String,
    pub md5: String,
    pub size: usize,
    pub duration: Duration,
}

impl FileElement {
    pub fn new(name: String, fid: String, md5: String, size: usize, duration: Duration) -> Self {
        Self {
            typee: "file",
            name,
            fid,
            md5,
            size,
            duration,
        }
    }
}

impl MessageElement for FileElement {}

/// 旧版引用回复(已弃用)，仅做一定程度的兼容
#[derive(Debug, Clone)]
pub struct ReplyElement {
    pub typee: &'static str,
    pub id: String,
}

impl ReplyElement {
    pub fn new(id: String) -> Self {
        Self { typee: "reply", id }
    }
}

impl MessageElement for ReplyElement {}
impl ChainElement for ReplyElement {}

#[derive(Debug, Clone)]
pub enum Sendable<T, A>
where
    T: MessageElement,
    A: MessageElement,
{
    String(String),
    Message(T),
    Array(Vec<A>),
}

/// 可引用回复的消息
#[derive(Debug, Clone)]
pub struct Quotable<T, A>
where
    T: MessageElement,
    A: MessageElement,
{
    pub user_id: u32,
    pub time: usize,
    pub seq: usize,
    /// 私聊回复必须
    pub rand: usize,
    /// 收到的引用回复永远是字符串
    pub message: Sendable<T, A>,
}

impl<T, A> Quotable<T, A>
where
    T: MessageElement,
    A: MessageElement,
{
    pub fn new(
        user_id: u32,
        time: usize,
        seq: usize,
        rand: usize,
        message: Sendable<T, A>,
    ) -> Self {
        Self {
            user_id,
            time,
            seq,
            rand,
            message,
        }
    }
}

/// 转发消息
#[derive(Debug, Clone)]
pub struct Forawrdable<T, A>
where
    T: MessageElement,
    A: MessageElement,
{
    pub user_id: u32,
    pub message: Sendable<T, A>,
    pub nickname: String,
    pub time: usize,
}

impl<T, A> Forawrdable<T, A>
where
    T: MessageElement,
    A: MessageElement,
{
    pub fn new(user_id: u32, message: Sendable<T, A>, nickname: String, time: usize) -> Self {
        Self {
            user_id,
            message,
            nickname,
            time,
        }
    }
}

// pub fn unescape_cq<S: AsRef<str>>(s: S) -> &'static str {
//     let s = s.as_ref();
//     match s {
//         "&#91;" => "[",
//         "&#93;" => "]",
//         "&amp;" => "&",
//         _ => "",
//     }
// }

// pub fn cq<S: AsRef<str>>(
//     s: S,
//     seq: Option<&str>,
//     equal: Option<&str>,
// ) -> Result<HashMap<String, Message>, serde_json::Error> {
//     let s = s.as_ref();
//     let seq = seq.unwrap_or(",");
//     let equal = equal.unwrap_or("=");

//     let mut result = HashMap::<String, Message>::new();
//     let splitted = s.split(seq);
//     for s in splitted {
//         if let Some(i) = s.find(equal) {
//             let k = s[0..i].to_string();
//             let v = &s[i + 1..]
//                 .replace("&#44;", ",")
//                 .replace("&#91;", "[")
//                 .replace("&#93;", "]")
//                 .replace("&amp;", "&");

//             if k != "text" {
//                 let json = serde_json::from_str(v)?;
//                 result.insert(k, Message::JSON(JSON::new(json)));
//             } else {
//                 result.insert(k, Message::Text(Text::new(v.to_string())));
//             };
//         }
//     }

//     Ok(result)
// }
