//! Source-file ownership, source IDs, spans, and line/column lookup.
//!
//! All later phases refer back to source through stable IDs and spans rather
//! than storing ad hoc filename and offset pairs.
