use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ECIESEerror {
    #[error("IO error")]
    IO(#[from] io::Error),

    #[error("tag check failure")]
    TagCheckFailed,

    #[error("invalid auth data")]
    InvalidAuthData,

    #[error("invalid ack data")]
    InvalidAckData,

    #[error("other")]
    Other(#[from] anyhow::Error),
}

impl From<ECIESEerror> for io::Error {
    fn from(value: ECIESEerror) -> Self {
        Self::new(io::ErrorKind::Other, format!("ECIES error: {:?}", value))
    }
}

impl From<secp256k1::Error> for ECIESEerror {
    fn from(value: secp256k1::Error) -> Self {
       Self::Other(value.into()) 
    }
}

impl From<rlp::DecoderError> for ECIESEerror {
    fn from(value: rlp::DecoderError) -> Self {
       Self::Other(value.into()) 
    }
}
