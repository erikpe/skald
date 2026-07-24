//! Source text to token-stream lowering.
//!
//! The lexer accepts the deliberately restricted token set documented in
//! `docs/language/GRAMMAR.md`. It recovers after invalid characters and
//! malformed decimal spellings, returning tokens and structured diagnostics
//! together.

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
