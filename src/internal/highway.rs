use std::{
    fmt::Display,
    future::Future,
    marker::PhantomPinned,
    net::SocketAddrV4,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};

use bytes::Buf;
use pin_project_lite::pin_project;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpSocket, TcpStream},
    select,
    time::Instant,
};

use crate::{
    client::Client,
    core::{
        io::WriteExt,
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
    fn ticket(&self) -> Result<&[u8], Error> {
        Err(Error::from(HighwayError::TicketNotProvided))
    }

    /// 根据实际需要继承
    fn ext(&self) -> Result<&[u8], Error> {
        Err(Error::from(HighwayError::ExtNotProvided))
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

async fn highway_transform<B, P>(client: &Client, body: B, params: &P) -> Result<(Vec<u8>), Error>
where
    P: HighwayUploadParameters,
    B: AsRef<[u8]>,
{
    let mut result = Vec::with_capacity(5120);

    let ext = if params.encrypt() {
        tea::encrypt(params.ext()?, &client.data().await.sig.bigdata.session_key)?
    } else {
        params.ext()?.to_vec()
    };
    let mut seq = rand::random::<u16>();

    let body = body.as_ref();
    let mut offset = 0;
    let limit = 1048576;
    while offset < body.len() {
        let chunk_start = offset;
        let chunk_end = if offset + limit > body.len() {
            body.len()
        } else {
            offset + limit
        };

        let chunk = &body[chunk_start..chunk_end];
        let head = protobuf::encode(&ProtobufObject::from([
            (
                1,
                ProtobufElement::Object(ProtobufObject::from([
                    (1, ProtobufElement::from(1)),
                    (
                        2,
                        ProtobufElement::from(client.data().await.uin.to_string()),
                    ),
                    (3, ProtobufElement::from("PicUp.DataUp")),
                    (4, ProtobufElement::from(seq)),
                    (6, ProtobufElement::from(client.data().await.apk.subid)),
                    (7, ProtobufElement::from(4096)),
                    (8, ProtobufElement::from(params.command_id() as u8)),
                    (10, ProtobufElement::from(2052)),
                ])),
            ),
            (
                2,
                ProtobufElement::Object(ProtobufObject::from([
                    (2, ProtobufElement::from(params.size())),
                    (3, ProtobufElement::from((offset) as isize)),
                    (4, ProtobufElement::from(chunk.len() as isize)),
                    (
                        6,
                        ProtobufElement::from(
                            params
                                .ticket()
                                .unwrap_or(&client.data().await.sig.bigdata.sig_session),
                        ),
                    ),
                    (8, ProtobufElement::from(md5::compute(chunk).0)),
                    (9, ProtobufElement::from(params.md5())),
                ])),
            ),
            (3, ProtobufElement::from(ext.clone())),
        ]))?;
        seq += 1;
        offset += chunk.len();

        let mut buf = Vec::with_capacity(9);
        buf.write_u8(40)?;
        buf.write_u32(head.len() as u32)?;
        buf.write_u32(chunk.len() as u32)?;

        result.write_bytes(&buf)?;
        result.write_bytes(head)?;
        result.write_bytes(chunk)?;
        result.write_u8(41)?;
    }

    Ok(result)
}

pin_project! {
    #[derive(Debug)]
    pub struct UploadProgress {
        stream: TcpStream,
        body: Vec<u8>,
        uploaded: usize,
        timeout: Option<Duration>,
        start: Instant,
        #[pin]
        _pin: PhantomPinned
    }
}

impl UploadProgress {
    fn new(stream: TcpStream, body: Vec<u8>, timeout: Option<Duration>) -> Self {
        Self {
            stream,
            body,
            uploaded: 0,
            timeout,
            start: Instant::now(),
            _pin: PhantomPinned,
        }
    }
}

impl Future for UploadProgress {
    type Output = Result<(usize, usize), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        if me
            .timeout
            .and_then(|t| Some(me.start.elapsed() >= t))
            .unwrap_or(false)
        {
            return Err(Error::from(HighwayError::UploadTimeout))?;
        }

        let stream = Pin::new(me.stream);
        ready!(stream.poll_write_ready(cx))?;

        if *me.uploaded >= me.body.len() {
            return Poll::Ready(Ok((me.body.len(), 0)));
        }

        let n = ready!(stream.poll_write(cx, &me.body[*me.uploaded..]))?;
        *me.uploaded += n;
        Poll::Ready(Ok((me.body.len(), *me.uploaded)))
    }
}

pub async fn highway_upload<B, P>(
    client: &Client,
    body: B,
    params: P,
    socket_addr: Option<SocketAddrV4>,
) -> Result<UploadProgress, Error>
where
    B: AsRef<[u8]>,
    P: HighwayUploadParameters,
{
    let socket_addr = socket_addr.or(client.data().await.sig.bigdata.socket_addr);

    if let Some(socket_addr) = socket_addr {
        let transformed = highway_transform(client, body, &params).await?;
        let tcp_stream = TcpSocket::new_v4()?.connect(socket_addr.into()).await?;
        Ok(UploadProgress::new(
            tcp_stream,
            transformed,
            params.timeout(),
        ))
    } else {
        Err(Error::from(HighwayError::UploadChannelNotExisted))
    }
}
