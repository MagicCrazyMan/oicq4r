use std::{collections::HashMap, path::PathBuf};

use rand::Rng;

use crate::core::{
    helper::BUF_2,
    protobuf::{ProtobufElement, ProtobufObject},
};

use super::element::{
    ATElement, ATTarget, BFaceElement, FaceElement, ImageElement, SFaceElement, TextElement,
    TryToFaceType, FACE_OLD_BUF,
};

#[derive(Debug)]
pub struct GroupMember {
    card: Option<String>,
    nickname: Option<String>,
}

impl GroupMember {
    pub fn card(&self) -> Option<&String> {
        self.card.as_ref()
    }

    pub fn nickname(&self) -> Option<&String> {
        self.nickname.as_ref()
    }

    pub fn try_to_string(&self) -> Option<&String> {
        self.card().or(self.nickname())
    }
}

static AT_BUF: [u8; 5] = [0, 1, 0, 0, 0];

#[derive(Debug)]
pub struct Converter<'a> {
    /// 是否为私聊，默认为 `false`
    dm: bool,
    /// 网络图片缓存路径
    cache_dir: PathBuf,
    /// 群员列表（用于 `@` 时查询 card）
    m_list: HashMap<u32, GroupMember>,

    ///
    is_chain: bool,
    objs: Vec<ProtobufObject>,

    /// 字符长度
    length: usize,
    /// 包含的图片（可能需要上传）
    images: Vec<&'a ImageElement>,
    /// 预览文字
    preview: String,

    /// 分片数据
    fragments: Vec<Vec<u8>>,
}

impl<'a> Converter<'a> {
    fn display(&mut self, text: &str, attr: Option<&[u8]>) {
        if !text.is_empty() {
            self.length += text.len();
            self.preview.push_str(text);

            let mut rsp = ProtobufObject::from([(1, ProtobufElement::from(text))]);

            if let Some(attr) = attr {
                rsp.insert(3, ProtobufElement::from(attr.as_ref()));
            }

            self.objs
                .push(ProtobufObject::from([(1, ProtobufElement::Object(rsp))]));
        }
    }

    fn text(&mut self, elem: &TextElement) {
        self.display(elem.text(), None);
    }

    fn at(&mut self, elem: &ATElement) {
        if let ATTarget::User(uid) = elem.target() {
            let (uid, flag, display) = if let Some(uid) = uid {
                let text = if elem.text().is_empty() {
                    self.m_list
                        .get(uid)
                        .and_then(|m| m.try_to_string())
                        .and_then(|s| Some(s.to_string()))
                        .unwrap_or(format!("@{uid}"))
                } else {
                    elem.text().to_string()
                };

                (*uid, 0, text)
            } else {
                (0, 1, "@全体成员".to_string())
            };

            if elem.is_dummy() {
                self.display(display.as_str(), None);
            } else {
                let mut attr6 = [0; 5 + 6 + 2];
                attr6[0] = AT_BUF[0];
                attr6[1] = AT_BUF[1];
                attr6[2] = AT_BUF[2];
                attr6[3] = AT_BUF[3];
                attr6[4] = AT_BUF[4];
                attr6[5] = display.len() as u8;
                attr6[6] = flag;
                let uid_bytes = uid.to_be_bytes();
                attr6[7] = uid_bytes[0];
                attr6[8] = uid_bytes[1];
                attr6[9] = uid_bytes[2];
                attr6[10] = uid_bytes[3];
                attr6[11] = BUF_2[0];
                attr6[12] = BUF_2[1];

                self.display(display.as_str(), Some(&attr6));
            }
        } else if let ATTarget::Channel(uid) = elem.target() {
            // 这里应该是有问题的，即使 text 为空也应该是直接把空字符串传过去，而不是生成一个 @谁谁谁
            let text = if !elem.text().is_empty() {
                elem.text().to_string()
            } else if let Some(uid) = uid {
                format!("@{uid}")
            } else {
                "@全体成员".to_string()
            };

            self.objs.push(ProtobufObject::from([(
                1,
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(text)),
                    (
                        12,
                        ProtobufElement::Object(ProtobufObject::from([
                            (3, ProtobufElement::from(2)),
                            (5, ProtobufElement::from(uid.unwrap_or(0))),
                        ])),
                    ),
                ])),
            )]));
        }
    }

    fn face(&mut self, elem: &FaceElement) {
        let id = elem.id();
        if id <= 0xff {
            let old = (id + 0x1441).to_be_bytes();

            self.objs.push(ProtobufObject::from([(
                2,
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(id)),
                    (2, ProtobufElement::from(old)),
                    (11, ProtobufElement::from(FACE_OLD_BUF)),
                ])),
            )]))
        } else {
            let text = if let Some(typee) = id.try_to_face_type() {
                typee.to_string()
            } else if let Some(text) = elem.text() {
                text.to_string()
            } else {
                format!("/{id}")
            };

            self.objs.push(ProtobufObject::from([(
                53,
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(33)),
                    (
                        2,
                        ProtobufElement::Object(ProtobufObject::from([
                            (1, ProtobufElement::from(id)),
                            (2, ProtobufElement::from(text.clone())),
                            (3, ProtobufElement::from(text)),
                        ])),
                    ),
                    (3, ProtobufElement::from(1)),
                ])),
            )]));
        }

        self.preview.push_str("[表情]");
    }

    fn s_face(&mut self, s_face: &SFaceElement) {
        let text = if let Some(text) = s_face.text() {
            format!("[{text}]")
        } else {
            format!("[{}]", s_face.id())
        };

        self.objs.push(ProtobufObject::from([(
            34,
            ProtobufElement::Object(ProtobufObject::from([
                (1, ProtobufElement::from(s_face.id())),
                (2, ProtobufElement::from(1)),
            ])),
        )]));
        self.display(text.as_str(), None);
    }

    fn b_face(&mut self, b_face: &BFaceElement) {
        let file = b_face.file();
        let text = if let Some(text) = b_face.text() {
            format!("[{}]", &text[..text.len().min(5)])
        } else {
            "[原创表情]".to_string()
        };

        self.display(&text, None);
        let mut o = ProtobufObject::from([
            (1, ProtobufElement::from(text)),
            (2, ProtobufElement::from(6)),
            (3, ProtobufElement::from(1)),
            (4, ProtobufElement::from(&file[0..16])),
            (5, ProtobufElement::from(&file[32..])),
            (6, ProtobufElement::from(3)),
            (7, ProtobufElement::from(&file[16..32])),
            (9, ProtobufElement::from(0)),
            (10, ProtobufElement::from(200)),
            (11, ProtobufElement::from(200)),
        ]);
        if let Some(magic) = b_face.magic() {
            o.insert(12, ProtobufElement::from(magic.clone()));
        }

        self.objs
            .push(ProtobufObject::from([(6, ProtobufElement::Object(o))]));
    }

    fn image(&mut self, elem: &ImageElement) {
        todo!()
        // let img = Image::new(elem, Some(self.dm), Some(&self.cache_dir));
        // self.images.push(img);

        // let o = if self.dm {
        //     ProtobufObject::from([
        //         (4, ProtobufElement::Object(elem.))
        //     ])
        // } else {

        // };
        // self.objs.push(value)
    }

    // fn flash
}
