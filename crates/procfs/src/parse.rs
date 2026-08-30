//! Numeric parsing helpers shared by `/proc` file parsers.
//!
//! Everything operates on `&[u8]` so files can be parsed from their raw
//! buffers without UTF-8 validation or intermediate `String`s. Two styles
//! are provided: strict token parsers (missing or malformed input is an
//! error) and the tolerant iterator parser used to skip malformed lines.

use crate::ProcResult;

/// Tolerantly parse the next token as a `u64`.
///
/// Returns `None` for a missing or malformed token so callers can skip
/// malformed entries (e.g. bad lines in `/proc/net/dev`) instead of failing
/// the whole parse.
pub fn parse_u64(token: Option<&[u8]>) -> ProcResult<u64> {
    lexical::parse(token.unwrap_or_default()).map_err(Into::into)
}

/// Strictly parse a token as a `u32`.
///
/// A missing token parses as empty input and fails with a lexical error, so
/// malformed snapshots are never reported as partial data.
pub fn parse_u32(token: Option<&[u8]>) -> ProcResult<u32> {
    lexical::parse(token.unwrap_or_default()).map_err(Into::into)
}

/// Strictly parse a token as an `f64`.
///
/// A missing token parses as empty input and fails with a lexical error, so
/// malformed snapshots are never reported as partial data.
pub fn parse_f64(token: Option<&[u8]>) -> ProcResult<f64> {
    lexical::parse(token.unwrap_or_default()).map_err(Into::into)
}
