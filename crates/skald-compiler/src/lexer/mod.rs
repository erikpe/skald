//! Source text to token-stream lowering for the first vertical slice.
//!
//! The lexer accepts the deliberately narrow token set documented in
//! `grammar/README.md`. It recovers after invalid characters and malformed
//! decimal spellings, returning tokens and structured diagnostics together.

mod dump;
mod numeric;
mod scanner;
mod token;

pub use dump::dump_tokens;
pub use scanner::{
    lex, LexOutput, MALFORMED_INTEGER_LITERAL, MALFORMED_NUMERIC_LITERAL, UNEXPECTED_CHARACTER,
};
pub use token::{Token, TokenKind};

#[cfg(test)]
mod tests;
