use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcError {
    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("lexical error: {0}")]
    LexicalError(#[from] lexical::Error),
}
