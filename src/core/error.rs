use std::fmt::Display;

use super::{jce::JceError, tlv::TlvError, tea::TeaError, network::NetworkError, base_client::ClientError, protobuf::ProtobufError};

#[derive(Debug)]
pub enum ErrorKind {
    JceError(JceError),
    TeaError(TeaError),
    TlvError(TlvError),
    ProtobufError(ProtobufError),
    NetworkError(NetworkError),
    ClientError(ClientError),
    StdTryFromSliceError(std::array::TryFromSliceError),
    StdFromUtf8Error(std::string::FromUtf8Error),
    StdIoError(std::io::Error),
    HyperError(hyper::Error),
    HyperHttpError(hyper::http::Error),
    TokioJoinError(tokio::task::JoinError),
    Flate2DecompressError(flate2::DecompressError),
    StringifyError(String),
}

#[derive(Debug)]
pub struct Error(ErrorKind);

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            ErrorKind::JceError(err) => err.fmt(f),
            ErrorKind::TeaError(err) => err.fmt(f),
            ErrorKind::TlvError(err) => err.fmt(f),
            ErrorKind::ProtobufError(err) => err.fmt(f),
            ErrorKind::NetworkError(err) => err.fmt(f),
            ErrorKind::ClientError(err) => err.fmt(f),
            ErrorKind::StdTryFromSliceError(err) => err.fmt(f),
            ErrorKind::StdFromUtf8Error(err) => err.fmt(f),
            ErrorKind::StdIoError(err) => err.fmt(f),
            ErrorKind::HyperError(err) => err.fmt(f),
            ErrorKind::HyperHttpError(err) => err.fmt(f),
            ErrorKind::TokioJoinError(err) => err.fmt(f),
            ErrorKind::Flate2DecompressError(err) => err.fmt(f),
            ErrorKind::StringifyError(msg) => f.write_str(msg),
        }
    }
}

impl Error {
    pub fn new(kind: ErrorKind) -> Self {
        Self(kind)
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.0
    }
}

impl From<JceError> for Error {
    fn from(err: JceError) -> Self {
        Self(ErrorKind::JceError(err))
    }
}

impl From<TeaError> for Error {
    fn from(err: TeaError) -> Self {
        Self(ErrorKind::TeaError(err))
    }
}

impl From<TlvError> for Error {
    fn from(err: TlvError) -> Self {
        Self(ErrorKind::TlvError(err))
    }
}

impl From<ProtobufError> for Error {
    fn from(err: ProtobufError) -> Self {
        Self(ErrorKind::ProtobufError(err))
    }
}

impl From<NetworkError> for Error {
    fn from(err: NetworkError) -> Self {
        Self(ErrorKind::NetworkError(err))
    }
}

impl From<ClientError> for Error {
    fn from(err: ClientError) -> Self {
        Self(ErrorKind::ClientError(err))
    }
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Self(ErrorKind::HyperError(err))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self(ErrorKind::StdIoError(err))
    }
}

impl From<hyper::http::Error> for Error {
    fn from(err: hyper::http::Error) -> Self {
        Self(ErrorKind::HyperHttpError(err))
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Self(ErrorKind::TokioJoinError(err))
    }
}

impl From<std::array::TryFromSliceError> for Error {
    fn from(err: std::array::TryFromSliceError) -> Self {
        Self(ErrorKind::StdTryFromSliceError(err))
    }
}

impl From<flate2::DecompressError> for Error {
    fn from(err: flate2::DecompressError) -> Self {
        Self(ErrorKind::Flate2DecompressError(err))
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self(ErrorKind::StdFromUtf8Error(err))
    }
}
