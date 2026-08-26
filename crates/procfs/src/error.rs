use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcError {
    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("parse float error: {0}")]
    FloatError(#[from] core::num::ParseFloatError),
    #[error("lexical error: {0}")]
    LexicalError(#[from] lexical::Error),
}
