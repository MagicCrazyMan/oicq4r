use super::jce::JceError;

#[derive(Debug)]
pub struct Error(String);

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error {
    pub fn illegal_data() -> Self {
        Error("illegal data".to_string())
    }
}


impl From<String> for Error {
    fn from(err: String) -> Self {
        Self(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Self(err.to_string())
    }
}

impl From<JceError> for Error {
    fn from(err: JceError) -> Self {
        Self(err.to_string())
    }
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<hyper::http::Error> for Error {
    fn from(err: hyper::http::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<tokio::io::Error> for Error {
    fn from(err: tokio::io::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Self(err.to_string())
    }
}

impl From<protobuf::Error> for Error {
    fn from(err: protobuf::Error) -> Self {
        Self(err.to_string())
    }
}