use std::{io::Cursor, path::PathBuf, time::Duration};

use rand::Rng;
use reqwest::{header::HeaderMap, IntoUrl};
use serde_json::Value;

use crate::{error::Error, core::protobuf::decode::DecodedObject};

pub trait MessageElement {}

pub trait ChainElement {}

/// @某人或全员
#[derive(Debug, Clone)]
pub struct TextElement {
    text: String,
}

impl TextElement {
    pub fn new(text: String) -> Self {
        Self { text }
    }

    pub fn text(&self) -> &str {
        self.text.as_ref()
    }
}

impl MessageElement for TextElement {}
impl ChainElement for TextElement {}

#[derive(Debug, Clone)]
pub enum ATTarget {
    /// @ 指定用户
    ///
    /// 如果 uid 不存在，则为 @全体
    User(Option<u32>),
    /// @ 频道
    ///
    /// 如果 uid 不存在，则为 @全体
    Channel(Option<u32>),
}

/// @某人或全员
#[derive(Debug, Clone)]
pub struct ATElement {
    target: ATTarget,
    text: String,
    /// 是否为假 AT
    dummy: bool,
}

impl ATElement {
    pub fn new(target: ATTarget, text: String, dummy: bool) -> Self {
        Self {
            target,
            text,
            dummy,
        }
    }

    pub fn target(&self) -> &ATTarget {
        &self.target
    }

    pub fn is_dummy(&self) -> bool {
        self.dummy
    }

    pub fn text(&self) -> &str {
        self.text.as_ref()
    }
}

impl MessageElement for ATElement {}
impl ChainElement for ATElement {}

pub static FACE_OLD_BUF: [u8; 8] = [0x00, 0x01, 0x00, 0x04, 0x52, 0xCC, 0xF5, 0xD0];
pub trait TryToFaceType {
    fn try_to_face_type(&self) -> Option<&'static str>;
}
impl TryToFaceType for u16 {
    fn try_to_face_type(&self) -> Option<&'static str> {
        let typee = match *self {
            0 => "惊讶",
            1 => "撇嘴",
            2 => "色",
            3 => "发呆",
            4 => "得意",
            5 => "流泪",
            6 => "害羞",
            7 => "闭嘴",
            8 => "睡",
            9 => "大哭",
            10 => "尴尬",
            11 => "发怒",
            12 => "调皮",
            13 => "呲牙",
            14 => "微笑",
            15 => "难过",
            16 => "酷",
            18 => "抓狂",
            19 => "吐",
            20 => "偷笑",
            21 => "可爱",
            22 => "白眼",
            23 => "傲慢",
            24 => "饥饿",
            25 => "困",
            26 => "惊恐",
            27 => "流汗",
            28 => "憨笑",
            29 => "悠闲",
            30 => "奋斗",
            31 => "咒骂",
            32 => "疑问",
            33 => "嘘",
            34 => "晕",
            35 => "折磨",
            36 => "衰",
            37 => "骷髅",
            38 => "敲打",
            39 => "再见",
            41 => "发抖",
            42 => "爱情",
            43 => "跳跳",
            46 => "猪头",
            49 => "拥抱",
            53 => "蛋糕",
            54 => "闪电",
            55 => "炸弹",
            56 => "刀",
            57 => "足球",
            59 => "便便",
            60 => "咖啡",
            61 => "饭",
            63 => "玫瑰",
            64 => "凋谢",
            66 => "爱心",
            67 => "心碎",
            69 => "礼物",
            74 => "太阳",
            75 => "月亮",
            76 => "赞",
            77 => "踩",
            78 => "握手",
            79 => "胜利",
            85 => "飞吻",
            86 => "怄火",
            89 => "西瓜",
            96 => "冷汗",
            97 => "擦汗",
            98 => "抠鼻",
            99 => "鼓掌",
            100 => "糗大了",
            101 => "坏笑",
            102 => "左哼哼",
            103 => "右哼哼",
            104 => "哈欠",
            105 => "鄙视",
            106 => "委屈",
            107 => "快哭了",
            108 => "阴险",
            109 => "亲亲",
            110 => "吓",
            111 => "可怜",
            112 => "菜刀",
            113 => "啤酒",
            114 => "篮球",
            115 => "乒乓",
            116 => "示爱",
            117 => "瓢虫",
            118 => "抱拳",
            119 => "勾引",
            120 => "拳头",
            121 => "差劲",
            122 => "爱你",
            123 => "不",
            124 => "好",
            125 => "转圈",
            126 => "磕头",
            127 => "回头",
            128 => "跳绳",
            129 => "挥手",
            130 => "激动",
            131 => "街舞",
            132 => "献吻",
            133 => "左太极",
            134 => "右太极",
            136 => "双喜",
            137 => "鞭炮",
            138 => "灯笼",
            140 => "K歌",
            144 => "喝彩",
            145 => "祈祷",
            146 => "爆筋",
            147 => "棒棒糖",
            148 => "喝奶",
            151 => "飞机",
            158 => "钞票",
            168 => "药",
            169 => "手枪",
            171 => "茶",
            172 => "眨眼睛",
            173 => "泪奔",
            174 => "无奈",
            175 => "卖萌",
            176 => "小纠结",
            177 => "喷血",
            178 => "斜眼笑",
            180 => "惊喜",
            181 => "骚扰",
            182 => "笑哭",
            183 => "我最美",
            184 => "河蟹",
            185 => "羊驼",
            187 => "幽灵",
            188 => "蛋",
            190 => "菊花",
            192 => "红包",
            193 => "大笑",
            194 => "不开心",
            197 => "冷漠",
            198 => "呃",
            199 => "好棒",
            200 => "拜托",
            201 => "点赞",
            202 => "无聊",
            203 => "托脸",
            204 => "吃",
            205 => "送花",
            206 => "害怕",
            207 => "花痴",
            208 => "小样儿",
            210 => "飙泪",
            211 => "我不看",
            212 => "托腮",
            214 => "啵啵",
            215 => "糊脸",
            216 => "拍头",
            217 => "扯一扯",
            218 => "舔一舔",
            219 => "蹭一蹭",
            220 => "拽炸天",
            221 => "顶呱呱",
            222 => "抱抱",
            223 => "暴击",
            224 => "开枪",
            225 => "撩一撩",
            226 => "拍桌",
            227 => "拍手",
            228 => "恭喜",
            229 => "干杯",
            230 => "嘲讽",
            231 => "哼",
            232 => "佛系",
            233 => "掐一掐",
            234 => "惊呆",
            235 => "颤抖",
            236 => "啃头",
            237 => "偷看",
            238 => "扇脸",
            239 => "原谅",
            240 => "喷脸",
            241 => "生日快乐",
            242 => "头撞击",
            243 => "甩头",
            244 => "扔狗",
            245 => "加油必胜",
            246 => "加油抱抱",
            247 => "口罩护体",
            260 => "/搬砖中",
            261 => "/忙到飞起",
            262 => "/脑阔疼",
            263 => "/沧桑",
            264 => "/捂脸",
            265 => "/辣眼睛",
            266 => "/哦哟",
            267 => "/头秃",
            268 => "/问号脸",
            269 => "/暗中观察",
            270 => "/emm",
            271 => "/吃瓜",
            272 => "/呵呵哒",
            273 => "/我酸了",
            274 => "/太南了",
            276 => "/辣椒酱",
            277 => "/汪汪",
            278 => "/汗",
            279 => "/打脸",
            280 => "/击掌",
            281 => "/无眼笑",
            282 => "/敬礼",
            283 => "/狂笑",
            284 => "/面无表情",
            285 => "/摸鱼",
            286 => "/魔鬼笑",
            287 => "/哦",
            288 => "/请",
            289 => "/睁眼",
            290 => "/敲开心",
            291 => "/震惊",
            292 => "/让我康康",
            293 => "/摸锦鲤",
            294 => "/期待",
            295 => "/拿到红包",
            296 => "/真好",
            297 => "/拜谢",
            298 => "/元宝",
            299 => "/牛啊",
            300 => "/胖三斤",
            301 => "/好闪",
            302 => "/左拜年",
            303 => "/右拜年",
            304 => "/红包包",
            305 => "/右亲亲",
            306 => "/牛气冲天",
            307 => "/喵喵",
            308 => "/求红包",
            309 => "/谢红包",
            310 => "/新年烟花",
            311 => "/打call",
            312 => "/变形",
            313 => "/嗑到了",
            314 => "/仔细分析",
            315 => "/加油",
            316 => "/我没事",
            317 => "/菜狗",
            318 => "/崇拜",
            319 => "/比心",
            320 => "/庆祝",
            321 => "/老色痞",
            322 => "/拒绝",
            323 => "/嫌弃",
            324 => "/吃糖",
            325 => "/惊吓",
            326 => "/生气",
            327 => "/加一",
            328 => "/错号",
            329 => "/对号",
            330 => "/完成",
            331 => "/明白",
            332 => "/举牌牌",
            333 => "/烟花",
            334 => "/虎虎生威",
            335 => "/绿马护体",
            336 => "/豹富",
            337 => "/花朵脸",
            338 => "/我想开了",
            339 => "/舔屏",
            340 => "/热化了",
            _ => "",
        };

        if typee.is_empty() {
            None
        } else {
            Some(typee)
        }
    }
}

/// Face 表情
#[derive(Debug, Clone)]
pub struct FaceElement {
    /// 0 ~ 324
    id: u16,
    text: Option<String>,
}

impl FaceElement {
    pub fn new(id: u16, text: Option<String>) -> Self {
        Self { id, text }
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    pub fn text(&self) -> Option<&String> {
        self.text.as_ref()
    }
}

impl MessageElement for FaceElement {}
impl ChainElement for FaceElement {}

/// SFace 表情
#[derive(Debug, Clone)]
pub struct SFaceElement {
    /// 不明
    id: u16,
    text: Option<String>,
}

impl SFaceElement {
    pub fn new(id: u16, text: Option<String>) -> Self {
        Self { id, text }
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    pub fn text(&self) -> Option<&String> {
        self.text.as_ref()
    }
}

impl MessageElement for SFaceElement {}
impl ChainElement for SFaceElement {}

/// 原创表情
#[derive(Debug, Clone)]
pub struct BFaceElement {
    /// 暂时只能发收到的 file
    file: Vec<u8>,
    text: Option<String>,
    /// 魔法表情携带的数据
    magic: Option<Vec<u8>>,
}

impl BFaceElement {
    pub fn new(file: Vec<u8>, text: Option<String>, magic: Option<Vec<u8>>) -> Self {
        Self { file, text, magic }
    }

    /// 骰子表情
    ///
    /// 骰子的结果会随机计算
    pub fn dice() -> Self {
        let number: u8 = rand::thread_rng().gen_range(0..6);

        Self::new(
            vec![
                0x48, 0x23, 0xd3, 0xad, 0xb1, 0x5d, 0xf0, 0x80, 0x14, 0xce, 0x5d, 0x67, 0x96, 0xb7,
                0x6e, 0xe1, 0x34, 0x30, 0x39, 0x65, 0x32, 0x61, 0x36, 0x39, 0x62, 0x31, 0x36, 0x39,
                0x31, 0x38, 0x66, 0x39, 0x11, 0x46,
            ],
            Some("骰子".to_string()),
            Some(vec![
                0x72,
                0x73,
                0x63,
                0x54,
                0x79,
                0x70,
                0x65,
                0x3f,
                0x31,
                0x3b,
                0x76,
                0x61,
                0x6c,
                0x75,
                0x65,
                0x3d,
                0x30 + number,
            ]),
        )
    }

    /// 猜拳表情
    ///
    /// 猜拳的结果会随机计算
    pub fn rps() -> Self {
        let number: u8 = rand::thread_rng().gen_range(0..3);

        Self::new(
            vec![
                0x83, 0xc8, 0xa2, 0x93, 0xae, 0x65, 0xca, 0x14, 0x0f, 0x34, 0x81, 0x20, 0xa7, 0x74,
                0x48, 0xee, 0x37, 0x64, 0x65, 0x33, 0x39, 0x66, 0x65, 0x62, 0x63, 0x66, 0x34, 0x35,
                0x65, 0x36, 0x64, 0x62, 0x11, 0x41,
            ],
            Some("猜拳".to_string()),
            Some(vec![
                0x72,
                0x73,
                0x63,
                0x54,
                0x79,
                0x70,
                0x65,
                0x3f,
                0x31,
                0x3b,
                0x76,
                0x61,
                0x6c,
                0x75,
                0x65,
                0x3d,
                0x30 + number,
            ]),
        )
    }

    pub fn file(&self) -> &[u8] {
        self.file.as_ref()
    }

    pub fn text(&self) -> Option<&String> {
        self.text.as_ref()
    }

    pub fn magic(&self) -> Option<&Vec<u8>> {
        self.magic.as_ref()
    }
}

impl MessageElement for BFaceElement {}
impl ChainElement for BFaceElement {}

/// 图片
#[derive(Debug, Clone)]
pub struct ImageElement {
    data: Vec<u8>,
    /// fid 后续从服务器中获取
    fid: Option<Vec<u8>>,
    /// 图片类型,有下列几种
    /// - jpg: `1000`
    /// - png: `1001`
    /// - webp: `1002`
    /// - bmp: `1005`
    /// - gif: `2000`
    /// - face: `4` (该类型仅允许从文件名中读取)
    /// - 无法解析或不存在将视为 jpg
    format: u32,
    /// 图片大小，此处会直接使用 data 的长度
    size: u64,
    /// 图片宽度
    width: u32,
    /// 图片高度
    height: u32,
    /// data 的 md5 哈希结果
    md5: [u8; 16],
    /// 网络图片是否使用缓存
    cache: bool,
    /// 是否作为表情发送
    as_face: bool,
    /// 是否显示下载原图按钮
    origin: bool,
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
            format,
            fid: None,
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

    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    pub fn format(&self) -> u32 {
        self.format
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn md5(&self) -> [u8; 16] {
        self.md5
    }

    pub fn is_cache(&self) -> bool {
        self.cache
    }

    pub fn is_as_face(&self) -> bool {
        self.as_face
    }

    pub fn is_origin(&self) -> bool {
        self.origin
    }

    pub fn fid(&self) -> Option<&Vec<u8>> {
        self.fid.as_ref()
    }

    pub fn set_fid(&mut self, fid: Option<Vec<u8>>) {
        self.fid = fid;
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

/// 可引用回复的消息
#[derive(Debug, Clone)]
pub struct Quotable<T>
where
    T: MessageElement,
{
    pub user_id: u32,
    pub time: usize,
    pub seq: usize,
    /// 私聊回复必须
    pub rand: usize,
    /// 收到的引用回复永远是字符串
    pub message: T,
}

impl<T> Quotable<T>
where
    T: MessageElement,
{
    pub fn new(user_id: u32, time: usize, seq: usize, rand: usize, message: T) -> Self {
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
pub struct Forawrdable<T>
where
    T: MessageElement,
{
    pub user_id: u32,
    pub message: T,
    pub nickname: String,
    pub time: usize,
}

impl<T> Forawrdable<T>
where
    T: MessageElement,
{
    pub fn new(user_id: u32, message: T, nickname: String, time: usize) -> Self {
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
