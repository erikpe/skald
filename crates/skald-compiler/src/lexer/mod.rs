//! Source text to token-stream lowering.
//!
//! The lexer accepts the deliberately restricted token set documented in
//! `docs/language/GRAMMAR.md`. It recovers after invalid characters and
//! malformed literal spellings, returning tokens and structured diagnostics
//! together.

mod byte;
mod dump;
mod escape;
mod numeric;
mod scanner;
mod string;
mod token;

pub(crate) use byte::decode_byte_literal;
pub use dump::dump_tokens;
pub use scanner::{
    lex, LexOutput, MALFORMED_BYTE_LITERAL, MALFORMED_INTEGER_LITERAL, MALFORMED_NUMERIC_LITERAL,
    MALFORMED_STRING_LITERAL, UNEXPECTED_CHARACTER,
};
pub(crate) use string::decode_string_literal;
pub use token::{Token, TokenKind};

#[cfg(test)]
mod tests;
