use std::{
    fmt::Display,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{
    core::protobuf::{ProtobufElement, ProtobufObject},
    ToHexString,
};

use super::element::ImageElement;

#[derive(Debug)]
pub enum ImageError {
    InvalidFilename,
}

impl std::error::Error for ImageError {}

impl Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::InvalidFilename => f.write_str("invalid filename for image."),
        }
    }
}

// trait FromExtension {
//     fn from_extension(&self) -> u32;
// }

// impl FromExtension for &str {
//     fn from_extension(&self) -> u32 {
//         match *self {
//             "jpg" => 1000,
//             "png" => 1001,
//             _=>""
//         }
//     }
// }

trait ToExtension {
    fn to_extension(&self) -> &'static str;
}

impl ToExtension for u32 {
    fn to_extension(&self) -> &'static str {
        match *self {
            3 => "png",
            4 => "face",
            1000 => "jpg",
            1001 => "png",
            1002 => "webp",
            1003 => "jpg",
            1005 => "bmp",
            2000 => "gif",
            2001 => "png",
            _ => "jpg",
        }
    }
}

/// 根据参数构造文件名
pub fn parse_filename_from_params(
    md5: [u8; 16],
    size: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    typee: Option<u32>,
) -> String {
    let md5_hex = md5.to_hex_string();
    let size = size.unwrap_or(0).to_string();
    let width = width.unwrap_or(0).to_string();
    let height = height.unwrap_or(0).to_string();
    let extension = typee.unwrap_or(0).to_extension();

    let mut filename = String::with_capacity(
        md5_hex.len() + size.len() + 1 + width.len() + 1 + height.len() + 1 + extension.len(),
    );
    filename.push_str(&md5_hex);
    filename.push_str(&size);
    filename.push_str("-");
    filename.push_str(&width);
    filename.push_str("-");
    filename.push_str(&height);
    filename.push_str(".");
    filename.push_str(&extension);

    filename
}

/// 根据文件名读取参数
fn parse_params_from_filename(filename: &str) -> Result<(&str, u64, u32, u32, &str), ImageError> {
    let mut md5_str: Option<&str> = None;
    let mut size: u64 = 0;
    let mut width: u32 = 0;
    let mut height: u32 = 0;

    let splitted = filename.split("-");
    for (i, partial) in splitted.enumerate() {
        if i == 0 {
            md5_str = Some(&partial[..32]);
            size = partial[32..].parse().unwrap_or(0);
        } else if i == 1 {
            width = partial.parse().unwrap_or(0);
        } else if i == 2 {
            height = partial.parse().unwrap_or(0)
        }
    }

    let md5_str = md5_str.ok_or(ImageError::InvalidFilename)?;
    let extension = filename.split(".").last().unwrap_or("jpg");

    Ok((md5_str, size, width, height, extension))
}

pub struct Image<'a> {
    element: &'a ImageElement,

    fid: ProtobufElement,
    pb_object: ProtobufObject,

    dm: bool,
    cache_dir: PathBuf,
}

impl Image<'_> {
    pub fn new<P>(element: ImageElement, dm: Option<bool>, cache_dir: Option<P>) -> Self
    where
        P: AsRef<Path>,
    {
        todo!()
    }

    fn update_params(&mut self) -> Result<(), ImageError> {
        // let () = parse_params_from_filename(filename)
        todo!()
    }

    fn update_pb(&mut self) {
        self.pb_object = if self.dm {
            ProtobufObject::from([
                (1, ProtobufElement::from(self.md5().to_hex_string())),
                (2, ProtobufElement::from(self.size())),
                (3, ProtobufElement::from(self.md5().to_hex_string())),
                (5, ProtobufElement::from(self.md5().to_hex_string())),
                (7, ProtobufElement::from(self.md5().to_hex_string())),
                (8, ProtobufElement::from(self.md5().to_hex_string())),
                (9, ProtobufElement::from(self.md5().to_hex_string())),
                (10, ProtobufElement::from(self.md5().to_hex_string())),
                (13, ProtobufElement::from(self.md5().to_hex_string())),
                (16, ProtobufElement::from(self.md5().to_hex_string())),
                (24, ProtobufElement::from(self.md5().to_hex_string())),
                (25, ProtobufElement::from(self.md5().to_hex_string())),
                (29, ProtobufElement::from(self.md5().to_hex_string())),
            ])
        } else {
            todo!()
        };
        todo!()
    }

    pub fn fid(&self) -> &ProtobufElement {
        &self.fid
    }

    pub fn set_fid(&mut self, fid: ProtobufElement) {
        self.fid = fid
    }

    pub fn size(&self) -> u64 {
        self.element.size
    }

    pub fn md5(&self) -> [u8; 16] {
        self.element.md5
    }

    pub fn origin(&self) -> bool {
        self.element.origin
    }

    pub fn width(&self) -> u32 {
        self.element.width
    }

    pub fn height(&self) -> u32 {
        self.element.height
    }

    pub fn format(&self) -> u32 {
        self.element.format
    }

    pub fn delete_tmp_file(&self) {
        
    }
}
