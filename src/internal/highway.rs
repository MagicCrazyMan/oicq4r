use std::{
    fmt::Display,
    future::Future,
    marker::PhantomPinned,
    net::SocketAddrV4,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};

use pin_project_lite::pin_project;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpSocket, TcpStream},
    sync::broadcast,
    time::Instant,
};

use crate::{
    client::Client,
    core::{
        protobuf::{self, ProtobufElement, ProtobufObject},
        tea,
    },
    error::Error,
};

#[derive(Debug)]
pub enum HighwayError {
    TicketNotProvided,
    ExtNotProvided,
    UploadChannelNotExisted,
    UploadTimeout,
}

impl Display for HighwayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HighwayError::UploadTimeout => f.write_str("upload timeout."),
            HighwayError::UploadChannelNotExisted => {
                f.write_str("no upload channel, please retry later.")
            }
            HighwayError::TicketNotProvided => f.write_str("ticket not provided."),
            HighwayError::ExtNotProvided => f.write_str("ext not provided."),
        }
    }
}

impl std::error::Error for HighwayError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    DmImage = 1,
    GroupImage = 2,
    SelfPortrait = 5,
    ShortVideo = 25,
    DmPTT = 26,
    MultiMsg = 27,
    GroupPTT = 29,
    OfflineFile = 69,
    GroupFile = 71,
    OCR = 76,
}

pub trait HighwayUploadParameters {
    fn command_id(&self) -> CommandId;

    fn size(&self) -> u64;

    fn md5(&self) -> [u8; 16];

    /// 根据实际需要继承
    fn ticket(&self) -> Result<&[u8], HighwayError> {
        Err(HighwayError::TicketNotProvided)
    }

    /// 根据实际需要继承
    fn ext(&self) -> Result<&[u8], HighwayError> {
        Err(HighwayError::ExtNotProvided)
    }

    /// 根据实际需要继承
    fn encrypt(&self) -> bool {
        false
    }

    /// 根据实际需要继承
    fn timeout(&self) -> Option<Duration> {
        None
    }
}

pin_project! {
    #[derive(Debug)]
    struct Transformer<R> {
        source: R,
        seq: u16,
        buf: Vec<u8>,
        // 总长度
        total: u64,
        // 已经被编码的长度
        transformed: u64,

        ticket:  Vec<u8>,
        command_id: CommandId,
        md5: [u8; 16],
        ext: Vec<u8>,

        uin: u32,
        subid: u32,

        #[pin]
        _pin: PhantomPinned
    }
}

impl<R: AsyncRead + Unpin> Transformer<R> {
    async fn new<P: HighwayUploadParameters>(
        client: &Client,
        params: &P,
        source: R,
    ) -> Result<Transformer<R>, Error> {
        let data = client.data().await;
        let ext = if params.encrypt() {
            tea::encrypt(params.ext()?, &data.sig.bigdata.session_key)?
        } else {
            params.ext()?.to_vec()
        };

        Ok(Self {
            source,
            seq: 25102,
            transformed: 0,
            buf: vec![0; 1024 * 1024],
            total: params.size(),
            ticket: params
                .ticket()
                .unwrap_or(&data.sig.bigdata.sig_session)
                .to_vec(),
            command_id: params.command_id(),
            md5: params.md5(),
            ext,
            uin: data.uin,
            subid: data.apk.subid,
            _pin: PhantomPinned,
        })
    }
}

impl<R: AsyncRead + Unpin> Transformer<R> {
    fn ended(&self) -> bool {
        self.total == self.transformed
    }
}

impl<R: AsyncRead + Unpin> Future for Transformer<R> {
    type Output = Result<Vec<u8>, std::io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        let mut raw_buf = ReadBuf::new(me.buf);
        ready!(Pin::new(me.source).poll_read(cx, &mut raw_buf))?;

        let filled = raw_buf.filled();
        if filled.len() == 0 {
            // 如果没有读取到任何数据，返回空列表
            Poll::Ready(Ok(vec![]))
        } else {
            // 进行编码后再输出
            let md5_filled = md5::compute(filled).0;
            let head = protobuf::encode(&ProtobufObject::from([
                (
                    1,
                    ProtobufElement::Object(ProtobufObject::from([
                        (1, ProtobufElement::from(1)),
                        (2, ProtobufElement::from(me.uin.to_string())),
                        (3, ProtobufElement::from("PicUp.DataUp")),
                        (4, ProtobufElement::from(*me.seq)),
                        (6, ProtobufElement::from(*me.subid)),
                        (7, ProtobufElement::from(4096)),
                        (8, ProtobufElement::from(*me.command_id as u8)),
                        (10, ProtobufElement::from(2052)),
                    ])),
                ),
                (
                    2,
                    ProtobufElement::Object(ProtobufObject::from([
                        (2, ProtobufElement::from(*me.total)),
                        (3, ProtobufElement::from(*me.transformed)),
                        (4, ProtobufElement::from(filled.len() as isize)),
                        (6, ProtobufElement::from(&me.ticket[..])),
                        (8, ProtobufElement::from(md5_filled)),
                        (9, ProtobufElement::from(&me.md5[..])),
                    ])),
                ),
                (3, ProtobufElement::from(&me.ext[..])),
            ]))
            .or(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid data, encode failure.",
            )))?;

            let mut buf = Vec::with_capacity(1 + 4 + 4 + head.len() + filled.len() + 1);
            buf.extend([40]);
            buf.extend(&(head.len() as u32).to_be_bytes());
            buf.extend(&(filled.len() as u32).to_be_bytes());
            buf.extend(&head[..]);
            buf.extend(&filled[..]);
            buf.extend([41]);

            *me.seq += 1;
            *me.transformed += filled.len() as u64;
            Poll::Ready(Ok(buf))
        }
    }
}

pin_project! {
    #[derive(Debug)]
    pub struct UploadProgress<S, R> {
        stream: S,
        transformer: Transformer<R>,

        // 本次已经编码，等待上传的数据
        queue: Vec<u8>,
        // 本次编码已将上传的长度
        uploaded: usize,

        received: [u8; 1024],
        wait_response: bool,

        timeout: Option<Duration>,
        start: Instant,

        progress_tx: broadcast::Sender<(usize, usize)>,
        #[pin]
        _pin: PhantomPinned
    }
}

impl<S: AsyncWrite + AsyncRead + Unpin, R: AsyncRead + Unpin> UploadProgress<S, R> {
    fn new(stream: S, transformer: Transformer<R>, timeout: Option<Duration>) -> Self {
        Self {
            stream,
            transformer,
            queue: vec![],
            uploaded: 0,
            received: [0; 1024],
            wait_response: false,
            timeout,
            start: Instant::now(),
            progress_tx: broadcast::channel(1).0,
            _pin: PhantomPinned,
        }
    }

    pub fn on_progress(&self) -> broadcast::Receiver<(usize, usize)> {
        self.progress_tx.subscribe()
    }
}

impl<S: AsyncWrite + AsyncRead + Unpin, R: AsyncRead + Unpin> Future for UploadProgress<S, R> {
    type Output = Result<(), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        if me
            .timeout
            .and_then(|t| Some(me.start.elapsed() >= t))
            .unwrap_or(false)
        {
            return Err(Error::from(HighwayError::UploadTimeout))?;
        }

        // 已经全部上传完，且所有数据已经编码完毕，完成本次上传任务
        if me.queue.len() == 0 && me.transformer.ended() {
            ready!(Pin::new(me.stream).poll_shutdown(cx))?;
            Poll::Ready(Ok(()))
        }
        // 检查当前已编码的数据是否已经上传完成，
        // 如果已经上传完成，则再去读取新的编码数据
        else if me.queue.len() == 0 {
            // SAFETY，因为实际上 Transformer 的数据只会通过 AsyncRead + Unpin 创建，所以此处是安全的
            unsafe {
                let transformer = Pin::new_unchecked(me.transformer);
                *me.queue = ready!(transformer.poll(cx)?);
            }

            cx.waker().wake_by_ref();
            Poll::Pending
        }
        // 读取编码数据和上传数据分为两个部分，以避免所有权冲突
        else {
            // 检查是否已经可以上传
            let stream = Pin::new(me.stream);
            // ready!(stream.poll_write_ready(cx))?;

            // 上传编码数据
            let n = ready!(stream.poll_write(cx, &me.queue[*me.uploaded..]))?;
            *me.uploaded += n;
            if me.queue.len() == *me.uploaded {
                me.queue.clear();
                *me.wait_response = true;
                *me.uploaded = 0;

                let _ = me.progress_tx.send((
                    me.transformer.total as usize,
                    me.transformer.transformed as usize,
                ));
            }

            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pub async fn highway_upload<R, P>(
    client: &Client,
    source: R,
    params: P,
    socket_addr: Option<SocketAddrV4>,
) -> Result<UploadProgress<TcpStream, R>, Error>
where
    R: AsyncRead + Unpin,
    P: HighwayUploadParameters,
{
    let socket_addr = socket_addr.or(client.data().await.sig.bigdata.socket_addr);

    if let Some(socket_addr) = socket_addr {
        let tcp_stream = TcpSocket::new_v4()?.connect(socket_addr.into()).await?;
        let transformer = Transformer::new(client, &params, source).await?;
        let uploader_progress = UploadProgress::new(tcp_stream, transformer, params.timeout());
        Ok(uploader_progress)
    } else {
        Err(Error::from(HighwayError::UploadChannelNotExisted))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;
    use crate::{client::Client, init_logger, tmp_dir};

    struct Params(Vec<u8>);

    impl HighwayUploadParameters for Params {
        fn command_id(&self) -> CommandId {
            CommandId::DmImage
        }

        fn size(&self) -> u64 {
            self.0.len() as u64
        }

        fn md5(&self) -> [u8; 16] {
            md5::compute(self.0.as_slice()).0
        }

        fn ticket(&self) -> Result<&[u8], HighwayError> {
            Ok("skfgjhdngjdknvbkdjfhgkerngjdfvndfkjngkjbngrkedgbnk".as_bytes())
        }

        fn ext(&self) -> Result<&[u8], HighwayError> {
            Ok("skfgjhdngjdknvbkdjfhgkerngjdfvndfkjngkjbngrkedgbnk".as_bytes())
        }
    }

    #[tokio::test]
    async fn test_upload() -> Result<(), Error> {
        init_logger()?;

        let pic_path = tmp_dir()?.join("qrcode.jpg");
        let mut file = std::fs::File::open(pic_path)?;
        let mut buf = Vec::with_capacity(2048);
        file.read_to_end(&mut buf)?;

        let client = Client::default(640279992).await;

        let params = Params(buf.clone());
        let mut aaa = buf.as_slice();
        let transformer = Transformer::new(&client, &params, &mut aaa).await?;

        // let mut receiver = Vec::with_capacity(5120);
        // let uploader_progress = UploadProgress::new(&mut receiver, transformer, params.timeout());

        // let mut rx = uploader_progress.on_progress();
        // tokio::spawn(async move {
        //     while let Ok((total, uploaded)) = rx.recv().await {
        //         println!(
        //             "total: {total}, uploaded: {uploaded}, percent: {:.4}%",
        //             uploaded
        //                 .ne(&0)
        //                 .then_some(uploaded as f64 / total as f64 * 100.0)
        //                 .unwrap_or(0.0)
        //         );
        //     }
        // });
        // uploader_progress.await?;

        // let mut f = std::fs::OpenOptions::new()
        //     .create(true)
        //     .append(false)
        //     .write(true)
        //     .open(tmp_dir()?.join("sdf"))?;
        // f.write_all(&mut receiver.as_slice())?;

        Ok(())
    }
}
