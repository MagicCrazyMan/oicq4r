use std::fmt::Display;

use crate::{core::{
    base_client::ClientError, jce::JceError, network::NetworkError, protobuf::ProtobufError,
    tea::TeaError, tlv::TlvError,
}, internal::highway::HighwayError};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum ErrorKind {
    JceError(JceError),
    TeaError(TeaError),
    TlvError(TlvError),
    ProtobufError(ProtobufError),
    NetworkError(NetworkError),
    ClientError(ClientError),
    HighwayError(HighwayError),
    StdTryFromSliceError(std::array::TryFromSliceError),
    StdFromUtf8Error(std::string::FromUtf8Error),
    StdIoError(std::io::Error),
    ReqwestError(reqwest::Error),
    TokioJoinError(tokio::task::JoinError),
    Flate2DecompressError(flate2::DecompressError),
    Base64DecodeError(base64::DecodeError),
    ImageError(image::error::ImageError),
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
            ErrorKind::HighwayError(err) => err.fmt(f),
            ErrorKind::StdTryFromSliceError(err) => err.fmt(f),
            ErrorKind::StdFromUtf8Error(err) => err.fmt(f),
            ErrorKind::StdIoError(err) => err.fmt(f),
            ErrorKind::ReqwestError(err) => err.fmt(f),
            ErrorKind::TokioJoinError(err) => err.fmt(f),
            ErrorKind::Flate2DecompressError(err) => err.fmt(f),
            ErrorKind::ImageError(err) => err.fmt(f),
            ErrorKind::StringifyError(msg) => f.write_str(msg),
            ErrorKind::Base64DecodeError(err) => err.fmt(f),
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

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Self(ErrorKind::StringifyError(err.to_string()))
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Self(ErrorKind::StringifyError(err))
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

impl From<HighwayError> for Error {
    fn from(err: HighwayError) -> Self {
        Self(ErrorKind::HighwayError(err))
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self(ErrorKind::ReqwestError(err))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self(ErrorKind::StdIoError(err))
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

impl From<base64::DecodeError> for Error {
    fn from(err: base64::DecodeError) -> Self {
        Self(ErrorKind::Base64DecodeError(err))
    }
}

impl From<image::error::ImageError> for Error {
    fn from(err: image::error::ImageError) -> Self {
        Self(ErrorKind::ImageError(err))
    }
}
