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
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpSocket, TcpStream,
    },
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
    UploadError(isize),
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
            HighwayError::UploadError(code) => {
                f.write_fmt(format_args!("upload failure (code: {})", code))
            }
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

/// HighwayUpload 配置参数
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
    /// Highway Upload Transformer，将输入的二进制数据分割并封装为上传格式
    #[derive(Debug)]
    struct Transformer<'a> {
        source: &'a [u8],
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

impl<'a> Transformer<'a> {
    async fn new<P: HighwayUploadParameters>(
        client: &'a Client,
        params: &'a P,
        source: &'a [u8],
    ) -> Result<Transformer<'a>, Error> {
        let data = client.data().await;
        let ext = if params.encrypt() {
            tea::encrypt(params.ext()?, &data.sig.bigdata.session_key)?
        } else {
            params.ext()?.to_vec()
        };

        Ok(Self {
            source,
            seq: rand::random::<u16>(),
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

    fn ended(&self) -> bool {
        self.total == self.transformed
    }
}

impl<'a> Future for Transformer<'a> {
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
    /// Highway Upload 中负责读取流中的服务器响应数据，用于确认传输数据结果。
    /// 上传百分比将通过此响应结果计算，而非通过本地已写入的数据量计算。
    #[derive(Debug)]
    pub struct UploadResponder<S> {
        stream: S,
        error_code: isize,
        finished: bool,

        buf: [u8; 1024],
        ready_buf: Vec<u8>,

        progress_tx: broadcast::Sender<f64>,
        #[pin]
        _pin: PhantomPinned
    }
}

impl<S> UploadResponder<S>
where
    S: AsyncRead + Unpin,
{
    fn new(stream: S) -> Self {
        Self {
            stream,
            error_code: 0,
            finished: false,
            buf: [0; 1024],
            ready_buf: Vec::with_capacity(5120),
            progress_tx: broadcast::channel(1).0,
            _pin: PhantomPinned,
        }
    }
}

impl<S> Future for UploadResponder<S>
where
    S: AsyncRead + Unpin,
{
    type Output = Result<(), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        if *me.finished {
            Poll::Ready(Ok(()))
        } else if *me.error_code != 0 {
            Poll::Ready(Err(HighwayError::UploadError(*me.error_code))?)
        } else {
            let mut buf = ReadBuf::new(me.buf);
            ready!(Pin::new(me.stream).poll_read(cx, &mut buf))?;

            let ready_buf = me.ready_buf;
            ready_buf.extend(buf.filled().iter());

            while ready_buf.len() >= 5 {
                let len = i32::from_be_bytes(ready_buf[1..5].try_into()?) as usize;

                if ready_buf.len() >= len + 10 {
                    let header = &mut &ready_buf[9..len + 9];
                    let mut rsp = protobuf::decode(header)?;

                    *me.error_code = rsp.try_remove(&3)?.try_into()?;
                    if *me.error_code != 0 {
                        // 上传出错
                        break;
                    } else {
                        let mut r2: ProtobufObject = rsp.try_remove(&2)?.try_into()?;

                        let r22: isize = r2.try_remove(&2)?.try_into()?;
                        let r23: isize = r2.try_remove(&3)?.try_into()?;
                        let r24: isize = r2.try_remove(&4)?.try_into()?;

                        let percent = (r23 as f64 + r24 as f64) / r22 as f64 * 100.0;
                        let _ = me.progress_tx.send(percent);

                        if percent >= 100.0 {
                            *me.finished = true;
                            break;
                        }
                    }

                    let _ = ready_buf.splice(..10 + len, []);
                } else {
                    break;
                }
            }

            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pin_project! {
    /// Highway Upload 中负责写入数据至服务器
    ///
    /// 该结构会同时生成一个 [`Transformer`] 将原始数据按长度编码成封装数据块再上传，
    /// 且编码模式为单块编码后单块上传，上传完成后再继续下一数据块的编码，以此交替直至上传完成。
    /// 不会立即就完成所有数据的编码，避免大文件上传时大幅占用内容空间。
    #[derive(Debug)]
    pub struct UploadProgress<'a, S> {
        stream: S,
        transformer: Transformer<'a>,

        // 本次已经编码，等待上传的数据
        queue: Vec<u8>,
        // 本次编码已将上传的长度
        uploaded: usize,

        timeout: Option<Duration>,
        start: Instant,

        #[pin]
        _pin: PhantomPinned
    }
}

impl<'a, S> UploadProgress<'a, S>
where
    S: AsyncWrite + Unpin,
{
    async fn new<P>(
        stream: S,
        client: &'a Client,
        params: &'a P,
        source: &'a [u8],
    ) -> Result<UploadProgress<'a, S>, Error>
    where
        P: HighwayUploadParameters,
    {
        let transformer = Transformer::new(client, params, source).await?;
        Ok(Self {
            stream,
            transformer,
            queue: vec![],
            uploaded: 0,
            timeout: params.timeout(),
            start: Instant::now(),
            _pin: PhantomPinned,
        })
    }
}

impl<'a, S> Future for UploadProgress<'a, S>
where
    S: AsyncWrite + Unpin,
{
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
            Poll::Ready(Ok(()))
        }
        // 检查当前已编码的数据是否已经上传完成，
        // 如果已经上传完成，则再去读取新的编码数据
        else if me.queue.len() == 0 {
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

            // 上传编码数据
            let n = ready!(stream.poll_write(cx, &me.queue[*me.uploaded..]))?;
            *me.uploaded += n;
            if me.queue.len() == *me.uploaded {
                me.queue.clear();
                *me.uploaded = 0;
            }

            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pin_project! {
    #[derive(Debug)]
    pub struct Uploader<'a> {
        responder: UploadResponder<OwnedReadHalf>,
        progress: UploadProgress<'a, OwnedWriteHalf>,
        #[pin]
        _pin: PhantomPinned
    }
}

impl<'a> Uploader<'a> {
    async fn new<P: HighwayUploadParameters>(
        client: &'a Client,
        stream: TcpStream,
        params: &'a P,
        source: &'a [u8],
    ) -> Result<Uploader<'a>, Error> {
        let (read, write) = stream.into_split();
        Ok(Self {
            responder: UploadResponder::new(read),
            progress: UploadProgress::new(write, client, params, source).await?,
            _pin: PhantomPinned,
        })
    }

    pub fn on_progress(&self) -> broadcast::Receiver<f64> {
        self.responder.progress_tx.subscribe()
    }

    pub async fn upload(self) -> Result<(), Error> {
        let (a, b) = tokio::join!(self.responder, self.progress,);

        a.and(b)
    }
}

pub async fn highway_upload<'a, P>(
    client: &'a Client,
    source: &'a [u8],
    params: &'a P,
    socket_addr: Option<SocketAddrV4>,
) -> Result<Uploader<'a>, Error>
where
    P: HighwayUploadParameters,
{
    let socket_addr = socket_addr.or(client.data().await.sig.bigdata.socket_addr);

    if let Some(socket_addr) = socket_addr {
        let tcp_stream = TcpSocket::new_v4()?.connect(socket_addr.into()).await?;
        let uploader = Uploader::new(client, tcp_stream, params, source).await?;
        Ok(uploader)
    } else {
        Err(Error::from(HighwayError::UploadChannelNotExisted))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

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

        let mut file = std::fs::File::open(tmp_dir()?.join("sjhs001.xyz_0002.jpg"))?;
        let mut buf = Vec::with_capacity(2048);
        file.read_to_end(&mut buf)?;
        let mut sliced = buf.as_slice();

        let client = Client::default(640279992).await;

        let param = Params(buf.clone());
        let uploader = highway_upload(
            &client,
            &mut sliced,
            &param,
            Some("127.0.0.1:1111".parse()?),
        )
        .await?;

        let mut rx = uploader.on_progress();
        tokio::spawn(async move {
            while let Ok(percent) = rx.recv().await {
                println!("percent: {:.4}%", percent);
            }
        });
        uploader.upload().await?;

        Ok(())
    }
}
