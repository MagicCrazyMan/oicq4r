pub mod decode;
pub mod encode;

#[cfg(test)]
mod test {
    use crate::{
        core::{
            io::WriteExt,
            protobuf::encode::{Element, Object},
        },
        error::Error,
    };

    #[test]
    fn test() -> Result<(), Error> {
        let mut buf = Vec::with_capacity(9);
        buf.write_u32(324372432)?;
        buf.write_bytes([0x00, 0x00, 0x01, 0x9e, 0x39])?;

        println!("{:02x?} {}", buf, buf.len());

        let encoded = Object::from([
            (1, Element::from(1152)),
            (2, Element::from(9)),
            (3, Element::from("242423424")),
            (4, Element::from(buf.as_slice())),
            (5, Element::from(213.435)),
            (
                6,
                Element::from(Object::from([
                    (1, Element::from(2)),
                    (2, Element::from("sdf")),
                ])),
            ),
            (
                7,
                Element::from(vec![Element::from(123), Element::from("345")]),
            ),
            (8, Element::from([Element::from(43), Element::from("dfg")])),
        ])
        .encode()?;

        println!("{:02x?} {}", encoded, encoded.len());

        Ok(())
    }
}
