use oicq4r::{
    core::protobuf::{decode, encode},
    error::Error,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let listener = TcpListener::bind("0.0.0.0:1111").await?;

    while let Ok((mut stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(false)
                .write(true)
                .open("./tmp/qqqq.jpg")
                .await
                .unwrap();
            let mut buf = Vec::with_capacity(1024 * 1024);

            let mut received_len = 0;
            loop {
                let (mut read, mut write) = stream.split();

                match read.read_buf(&mut buf).await {
                    Ok(len) => {
                        println!("received: {}", len);

                        if buf.len() >= 9 {
                            let reader = &mut &buf[1..];

                            let head_len = reader.read_u32().await.unwrap() as usize;
                            let chunk_len = reader.read_u32().await.unwrap() as usize;

                            if buf.len() >= 1 + 4 + 4 + head_len + chunk_len + 1 {
                                let mut head_buf = vec![0; head_len];
                                reader.read_exact(&mut head_buf).await.unwrap();

                                let mut head =
                                    decode::Object::decode(&mut head_buf.as_slice()).unwrap();
                                let mut metadata: decode::Object =
                                    head.try_remove(&2).unwrap().try_into().unwrap();
                                let total_len: i128 =
                                    metadata.try_remove(&2).unwrap().try_into().unwrap();

                                let encoded = {
                                    let nested = encode::Object::from([
                                        (2, encode::Element::from(total_len)),
                                        (3, encode::Element::from(received_len)),
                                        (4, encode::Element::from(chunk_len)),
                                    ]);
                                    encode::Object::from([
                                        (
                                            2,
                                            encode::Element::from(
                                                nested.encode().unwrap().as_slice(),
                                            ),
                                        ),
                                        (3, encode::Element::from(0)),
                                    ])
                                    .encode()
                                    .unwrap()
                                };

                                write.write_u8(0).await.unwrap();
                                write.write_u32(encoded.len() as u32).await.unwrap();
                                write.write_u32(0).await.unwrap();
                                write.write_all(&mut encoded.as_slice()).await.unwrap();
                                write.write_u8(0).await.unwrap();
                                println!("sent: {}", 1 + 4 + 4 + 1 + encoded.len());

                                let mut a = Vec::with_capacity(1024);
                                a.write_u8(0).await.unwrap();
                                a.write_u32(encoded.len() as u32).await.unwrap();
                                a.write_u32(0).await.unwrap();
                                a.write_all(&mut encoded.as_slice()).await.unwrap();
                                a.write_u8(0).await.unwrap();
                                println!("{:x?}", a);

                                received_len += chunk_len;

                                let mut chunk_buf = vec![0; chunk_len];
                                reader.read_exact(&mut chunk_buf).await.unwrap();
                                file.write_all(&mut chunk_buf.as_slice()).await.unwrap();

                                drop(reader);
                                let _ = buf.splice(..1 + 4 + 4 + head_len + chunk_len + 1, []);

                                if received_len == total_len as usize {
                                    break;
                                }
                            }
                        }
                    }
                    Err(err) => println!("error: {}", err),
                }
            }
        });
    }
    Ok(())
}
