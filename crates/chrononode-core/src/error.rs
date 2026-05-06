use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Proof error: {0}")]
    Proof(String),

    #[error("Adapter error: {0}")]
    Adapter(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Serialization(e.to_string())
    }
}

impl From<prost::EncodeError> for CoreError {
    fn from(e: prost::EncodeError) -> Self {
        CoreError::Serialization(e.to_string())
    }
}

impl From<prost::DecodeError> for CoreError {
    fn from(e: prost::DecodeError) -> Self {
        CoreError::Deserialization(e.to_string())
    }
}
