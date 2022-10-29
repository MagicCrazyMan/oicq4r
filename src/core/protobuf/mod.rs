/// Protobuf 的编解码采用各自采用两种不同的处理方式
/// 

pub mod decode;
pub mod encode;

#[cfg(test)]
mod test {
    use crate::{
        core::{io::WriteExt, protobuf::encode::{EncodedObject, EncodeProtobuf}},
        error::Error,
        to_protobuf,
    };

    #[test]
    fn test() -> Result<(), Error> {
        let mut buf = Vec::with_capacity(9);
        buf.write_u32(324372432)?;
        buf.write_bytes([0x00, 0x00, 0x01, 0x9e, 0x39])?;

        println!("{:02x?} {}", buf, buf.len());

        let encoded = EncodedObject::from([
            (1, to_protobuf!(1152)),
            (2, to_protobuf!(9)),
            (3, to_protobuf!("242423424")),
            (4, to_protobuf!(buf.as_slice())),
            (5, to_protobuf!(213.435)),
            (
                6,
                to_protobuf!(&EncodedObject::from([
                    (1, to_protobuf!(2)),
                    (2, to_protobuf!("sdf"))
                ])),
            ),
            (
                7,
                to_protobuf!(vec![to_protobuf!(123), to_protobuf!("345")]),
            ),
            (
                8,
                to_protobuf!([to_protobuf!(43), to_protobuf!("dfg")]),
            ),
        ])
        .encode()?;

        println!("{:02x?} {}", encoded, encoded.len());

        Ok(())
    }
}
