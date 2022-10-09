use std::array::TryFromSliceError;

use super::jce::JceError;

#[derive(Debug, Clone)]
pub struct CommonError(String);

impl std::error::Error for CommonError {}

impl std::fmt::Display for CommonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl CommonError {
    pub fn new<T: AsRef<str>>(message: T) -> Self {
        Self(message.as_ref().to_string())
    }

    pub fn illegal_data() -> Self {
        CommonError::new("illegal data")
    }

    pub fn bad_token() -> Self {
        CommonError::new("bad token")
    }

    pub fn not_registered() -> Self {
        CommonError::new("not registered")
    }

    pub fn tag_not_existed(tag: u16) -> Self {
        CommonError::new(format!("tag {:x} not existed", tag))
    }
}

impl From<String> for CommonError {
    fn from(err: String) -> Self {
        Self(err)
    }
}

impl From<&str> for CommonError {
    fn from(err: &str) -> Self {
        Self(err.to_string())
    }
}

impl From<JceError> for CommonError {
    fn from(err: JceError) -> Self {
        Self(err.to_string())
    }
}

impl From<hyper::Error> for CommonError {
    fn from(err: hyper::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<hyper::http::Error> for CommonError {
    fn from(err: hyper::http::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<tokio::io::Error> for CommonError {
    fn from(err: tokio::io::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<tokio::task::JoinError> for CommonError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self(err.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for CommonError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        Self(err.to_string())
    }
}

impl<T> From<tokio::sync::watch::error::SendError<T>> for CommonError {
    fn from(_: tokio::sync::watch::error::SendError<T>) -> Self {
        Self("channel closed".to_string())
    }
}

impl From<tokio::sync::watch::error::RecvError> for CommonError {
    fn from(err: tokio::sync::watch::error::RecvError) -> Self {
        Self(err.to_string())
    }
}

impl<T> From<tokio::sync::broadcast::error::SendError<T>> for CommonError {
    fn from(_: tokio::sync::broadcast::error::SendError<T>) -> Self {
        Self("channel closed".to_string())
    }
}

impl From<tokio::sync::broadcast::error::RecvError> for CommonError {
    fn from(err: tokio::sync::broadcast::error::RecvError) -> Self {
        Self(err.to_string())
    }
}

impl From<std::time::SystemTimeError> for CommonError {
    fn from(err: std::time::SystemTimeError) -> Self {
        Self(err.to_string())
    }
}

impl From<TryFromSliceError> for CommonError {
    fn from(err: TryFromSliceError) -> Self {
        Self(err.to_string())
    }
}

impl From<flate2::DecompressError> for CommonError {
    fn from(err: flate2::DecompressError) -> Self {
        Self(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for CommonError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self(err.to_string())
    }
}

impl From<protobuf::Error> for CommonError {
    fn from(err: protobuf::Error) -> Self {
        Self(err.to_string())
    }
}
