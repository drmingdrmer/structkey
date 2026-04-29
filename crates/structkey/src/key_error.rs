use std::string::FromUtf8Error;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),

    #[error("Expect {i}-th segment to be '{expect}', but: '{got}'")]
    InvalidSegment {
        i: usize,
        expect: String,
        got: String,
    },

    #[error("Expect {i}-th segment to be non-empty")]
    EmptySegment { i: usize },

    #[error("Expect {expect} segments, but: '{got}'")]
    WrongNumberOfSegments { expect: usize, got: String },

    #[error("Expect at least {expect} segments, but {actual} segments found")]
    AtleastSegments { expect: usize, actual: usize },

    #[error("Invalid id string: '{s}': {reason}")]
    InvalidId { s: String, reason: String },

    #[error("Unknown key prefix: '{prefix}'")]
    UnknownPrefix { prefix: String },

    #[error("Invalid percent-encoded sequence at byte {pos} in '{input}'")]
    InvalidEscape { pos: usize, input: String },

    #[error("`{type_name}` does not support decoding from a key string")]
    NotDecodable { type_name: &'static str },
}
