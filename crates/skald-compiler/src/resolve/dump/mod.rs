//! Deterministic textual rendering of the resolved program.

mod body;
mod declarations;
mod expression;
mod generic;
mod object;
mod program;
mod type_name;

use std::fmt::Write;

use crate::{
    dump_format::{write_indentation, write_span},
    source::Span,
};

use super::ir::*;

pub fn dump_resolved(program: &ResolvedProgram) -> String {
    program::dump_program(program)
}

struct ResolvedDumper<'program> {
    output: String,
    indentation: usize,
    program: &'program ResolvedProgram,
}

impl<'program> ResolvedDumper<'program> {
    fn new(program: &'program ResolvedProgram) -> Self {
        Self {
            output: String::new(),
            indentation: 0,
            program,
        }
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn raw_line(&mut self, text: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{text}");
    }

    fn line(&mut self, name: &str, span: Span) {
        self.write_indentation();
        self.output.push_str(name);
        write_span(&mut self.output, span);
        self.output.push('\n');
    }

    fn write_indentation(&mut self) {
        write_indentation(&mut self.output, self.indentation);
    }

    fn indented(&mut self, write_contents: impl FnOnce(&mut Self)) {
        self.indentation += 1;
        write_contents(self);
        self.indentation -= 1;
    }
}
