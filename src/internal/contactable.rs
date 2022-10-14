use async_trait::async_trait;

use crate::{
    client::Client,
    core::protobuf::{self, ProtobufElement, ProtobufObject},
    error::Error,
    message::image::Image,
    ToHexString,
};

#[async_trait]
pub trait Contractable {
    /// 对方 QQ 号或 QQ 群号
    fn target(&self) -> u32;

    fn client(&self) -> &Client;

    /// 是否为私聊
    fn dm(&self) -> bool;

    /// 获取私聊图片 fid
    async fn off_pic_up(&self, images: &[Image]) -> Result<ProtobufElement, Error> {
        let client = self.client();
        let mut data = client.data().await;

        let mut elements = Vec::with_capacity(images.len());
        for (i, image) in images.into_iter().enumerate() {
            let md5 = image.md5();
            let md5_str = md5.to_hex_string();
            let object = ProtobufObject::from([
                (1, ProtobufElement::from(data.uin)),
                (2, ProtobufElement::from(self.target())),
                (3, ProtobufElement::from(0)),
                (4, ProtobufElement::from(md5)),
                (5, ProtobufElement::from(image.size())),
                (6, ProtobufElement::from(md5_str)),
                (7, ProtobufElement::from(5)),
                (8, ProtobufElement::from(9)),
                (9, ProtobufElement::from(0)),
                (10, ProtobufElement::from(0)),
                (11, ProtobufElement::from(0)),
                (12, ProtobufElement::from(1)),
                (
                    13,
                    ProtobufElement::from(image.origin().then_some(1).unwrap_or(0)),
                ),
                (14, ProtobufElement::from(image.width())),
                (15, ProtobufElement::from(image.height())),
                (16, ProtobufElement::from(image.format())),
                (17, ProtobufElement::from(data.apk.version)),
                (22, ProtobufElement::from(0)),
            ]);

            elements.push((i as u32, ProtobufElement::Object(object)));
        }

        let pb_obj = ProtobufObject::from(elements);
        let body = protobuf::encode(&ProtobufObject::from([
            (1, ProtobufElement::from(1)),
            (2, ProtobufElement::from(pb_obj)),
        ]))?;

        let request = data.build_uni_request("LongConn.OffPicUp", body, None)?;
        drop(data);

        let response = client.send_registered_request(request, None).await?.await?;
        let mut decoded = protobuf::decode(&mut response.as_slice())?;

        Ok(decoded.try_remove(&2)?)
    }

    /// 获取群聊图片fid
    async fn group_pic_up(&self, images: &[Image]) -> Result<ProtobufElement, Error> {
        let client = self.client();
        let mut data = client.data().await;

        let mut elements = Vec::with_capacity(images.len());
        for (i, image) in images.into_iter().enumerate() {
            let md5 = image.md5();
            let md5_str = md5.map(|b| format!("{:02x}", b)).join("");
            let object = ProtobufObject::from([
                (1, ProtobufElement::from(data.uin)),
                (2, ProtobufElement::from(self.target())),
                (3, ProtobufElement::from(0)),
                (4, ProtobufElement::from(md5)),
                (5, ProtobufElement::from(image.size())),
                (6, ProtobufElement::from(md5_str)),
                (7, ProtobufElement::from(5)),
                (8, ProtobufElement::from(9)),
                (9, ProtobufElement::from(1)),
                (10, ProtobufElement::from(image.width())),
                (11, ProtobufElement::from(image.height())),
                (12, ProtobufElement::from(image.format())),
                (13, ProtobufElement::from(data.apk.version)),
                (14, ProtobufElement::from(0)),
                (15, ProtobufElement::from(1052)),
                (
                    16,
                    ProtobufElement::from(image.origin().then_some(1).unwrap_or(0)),
                ),
                (18, ProtobufElement::from(0)),
                (19, ProtobufElement::from(0)),
            ]);

            elements.push((i as u32, ProtobufElement::Object(object)));
        }

        let pb_obj = ProtobufObject::from(elements);
        let body = protobuf::encode(&ProtobufObject::from([
            (1, ProtobufElement::from(3)),
            (2, ProtobufElement::from(1)),
            (3, ProtobufElement::from(pb_obj)),
        ]))?;

        let request = data.build_uni_request("ImgStore.GroupPicUp", body, None)?;
        drop(data);

        let response = client.send_registered_request(request, None).await?.await?;
        let mut decoded = protobuf::decode(&mut response.as_slice())?;

        Ok(decoded.try_remove(&3)?)
    }

    async fn upload_image(&self, image: &mut Image, mut rsp: ProtobufObject) -> Result<(), Error> {
        let j = self.dm().then_some(1).unwrap_or(0);

        let tmp: isize = rsp.try_remove(&(2 + j))?.try_into()?;
        if tmp != 0 {
            let msg: String = rsp.try_remove(&(3 + j))?.try_into()?;
            Err(Error::from(msg))
        } else {
            let fid = rsp.try_remove(&(9 + j))?;
            image.set_fid(fid);

            let tmp: isize = rsp.try_remove(&(4 + j))?.try_into()?;
            if tmp != 0 {
                // 上传成功
                return Ok(());
            }
            
            Ok(())
        }
    }

    async fn upload_images(&self, images: &[Image]) -> Result<(), Error> {
        for chunk in images.chunks(20) {
            let rsp = if self.dm() {
                self.off_pic_up(chunk).await?
            } else {
                self.group_pic_up(chunk).await?
            };

            let rsp = if let ProtobufElement::Array(e) = rsp {
                e
            } else {
                vec![rsp]
            };

            for (image, rsp) in chunk.into_iter().zip(rsp.into_iter()) {
                let rsp: ProtobufObject = rsp.try_into()?;
            }
        }

        todo!()
    }
}
