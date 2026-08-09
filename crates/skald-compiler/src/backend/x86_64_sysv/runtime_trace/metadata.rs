//! Requested-only runtime-trace metadata planning for Linux x86-64.

use std::{collections::BTreeMap, path::Path};

use crate::{
    backend::{BackendError, BackendInput, RuntimeTracePolicy, Target},
    identity::CallableId,
    mir::MirProgram,
    source::{SourceDatabase, Span},
};

use super::super::{
    machine::{
        AssemblyRuntimeTraceMetadata, AssemblyTraceByteString, AssemblyTraceContext,
        AssemblyTraceLocation,
    },
    symbol,
};

use super::names;

pub(in crate::backend::x86_64_sysv) enum Metadata<'input> {
    Omitted,
    Enabled(EnabledMetadata<'input>),
}

impl<'input> Metadata<'input> {
    pub(in crate::backend::x86_64_sysv) fn new(input: BackendInput<'input>) -> Self {
        match input.runtime_trace() {
            RuntimeTracePolicy::Enabled => Self::Enabled(EnabledMetadata {
                program: input.program(),
                sources: input
                    .sources()
                    .expect("enabled backend input must retain its source database"),
                contexts: BTreeMap::new(),
                locations: BTreeMap::new(),
            }),
            RuntimeTracePolicy::Omitted => Self::Omitted,
        }
    }

    pub(in crate::backend::x86_64_sysv) fn request_location(
        &mut self,
        callable: CallableId,
        span: Span,
    ) -> Result<Option<String>, BackendError> {
        match self {
            Self::Omitted => Ok(None),
            Self::Enabled(metadata) => metadata.request_location(callable, span).map(Some),
        }
    }

    pub(in crate::backend::x86_64_sysv) fn finish(self) -> AssemblyRuntimeTraceMetadata {
        match self {
            Self::Omitted => AssemblyRuntimeTraceMetadata::default(),
            Self::Enabled(metadata) => metadata.finish(),
        }
    }
}

pub(in crate::backend::x86_64_sysv) struct EnabledMetadata<'input> {
    program: &'input MirProgram,
    sources: &'input SourceDatabase,
    contexts: BTreeMap<CallableId, PendingContext>,
    locations: BTreeMap<LocationKey, PendingLocation>,
}

struct PendingContext {
    callable: CallableId,
    symbol: String,
    name: Vec<u8>,
    path: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LocationKey {
    callable: CallableId,
    line: u64,
    column: u64,
}

struct PendingLocation {
    symbol: String,
}

impl EnabledMetadata<'_> {
    fn request_location(
        &mut self,
        callable: CallableId,
        span: Span,
    ) -> Result<String, BackendError> {
        if !self
            .program
            .executable_definitions()
            .any(|definition| definition.callable() == callable)
        {
            return Err(error(
                Some(callable),
                "runtime trace location requested for a callable without an executable body",
            ));
        }

        let module = names::module_for_callable(self.program, callable)?;
        let provenance = self.program.modules.get(module).ok_or_else(|| {
            error(
                Some(callable),
                "runtime trace callable has no module provenance",
            )
        })?;
        if provenance.source_id() != span.source_id() {
            return Err(error(
                Some(callable),
                "runtime trace span belongs to a different source than its callable",
            ));
        }
        let source = self.sources.get(span.source_id()).ok_or_else(|| {
            error(
                Some(callable),
                "runtime trace span source is absent from the source database",
            )
        })?;
        let location = source.location(span.range().start()).ok_or_else(|| {
            error(
                Some(callable),
                "runtime trace span start is not a valid source location",
            )
        })?;
        let line = u64::try_from(location.line).map_err(|_| {
            error(
                Some(callable),
                "runtime trace source line cannot be represented by the runtime ABI",
            )
        })?;
        let column = u64::try_from(location.column).map_err(|_| {
            error(
                Some(callable),
                "runtime trace source column cannot be represented by the runtime ABI",
            )
        })?;

        if !self.contexts.contains_key(&callable) {
            self.contexts.insert(
                callable,
                PendingContext {
                    callable,
                    symbol: symbol::trace_context(self.program, callable),
                    name: names::callable(self.program, callable)?.into_bytes(),
                    path: escape_path(provenance.source_location().trace_source_path()),
                },
            );
        }
        let key = LocationKey {
            callable,
            line,
            column,
        };
        let location = self
            .locations
            .entry(key)
            .or_insert_with(|| PendingLocation {
                symbol: symbol::trace_location(self.program, callable, line, column),
            });
        Ok(location.symbol.clone())
    }

    fn finish(self) -> AssemblyRuntimeTraceMetadata {
        let mut contexts = self.contexts.into_values().collect::<Vec<_>>();
        contexts.sort_by(|left, right| {
            (&left.name, &left.path, left.callable).cmp(&(&right.name, &right.path, right.callable))
        });

        let context_by_callable = contexts
            .iter()
            .map(|context| (context.callable, context))
            .collect::<BTreeMap<_, _>>();
        let mut locations = self.locations.into_iter().collect::<Vec<_>>();
        locations.sort_by(|(left, _), (right, _)| {
            let left_context = &context_by_callable[&left.callable];
            let right_context = &context_by_callable[&right.callable];
            (
                &left_context.name,
                &left_context.path,
                left.line,
                left.column,
                left.callable,
            )
                .cmp(&(
                    &right_context.name,
                    &right_context.path,
                    right.line,
                    right.column,
                    right.callable,
                ))
        });

        let mut bytes = Vec::new();
        let mut symbols = BTreeMap::new();
        for context in &contexts {
            for value in [&context.name, &context.path] {
                if !symbols.contains_key(value) {
                    let byte_symbol = symbol::trace_byte_string(bytes.len());
                    symbols.insert(value.clone(), byte_symbol);
                    bytes.push(value.clone());
                }
            }
        }

        AssemblyRuntimeTraceMetadata {
            strings: bytes
                .into_iter()
                .map(|bytes| AssemblyTraceByteString {
                    symbol: symbols[&bytes].clone(),
                    bytes,
                })
                .collect(),
            contexts: contexts
                .iter()
                .map(|context| AssemblyTraceContext {
                    symbol: context.symbol.clone(),
                    name_symbol: symbols[&context.name].clone(),
                    name_length: u64::try_from(context.name.len())
                        .expect("x86-64 trace name length must fit u64"),
                    path_symbol: symbols[&context.path].clone(),
                    path_length: u64::try_from(context.path.len())
                        .expect("x86-64 trace path length must fit u64"),
                })
                .collect(),
            locations: locations
                .into_iter()
                .map(|(key, location)| AssemblyTraceLocation {
                    symbol: location.symbol,
                    context_symbol: context_by_callable[&key.callable].symbol.clone(),
                    line: key.line,
                    column: key.column,
                })
                .collect(),
        }
    }
}

fn escape_path(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        escape_path_bytes(path.as_os_str().as_bytes())
    }
    #[cfg(not(unix))]
    {
        escape_path_bytes(path.to_string_lossy().as_bytes())
    }
}

pub(in crate::backend::x86_64_sysv) fn escape_path_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(bytes.len());
    let mut remaining = bytes;
    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(valid) => {
                append_valid_path_bytes(&mut escaped, valid.as_bytes());
                break;
            }
            Err(invalid) => {
                let valid_length = invalid.valid_up_to();
                append_valid_path_bytes(&mut escaped, &remaining[..valid_length]);
                let invalid_length = invalid.error_len().unwrap_or(1);
                for byte in &remaining[valid_length..valid_length + invalid_length] {
                    append_hex_escape(&mut escaped, *byte);
                }
                remaining = &remaining[valid_length + invalid_length..];
            }
        }
    }
    escaped
}

fn append_valid_path_bytes(escaped: &mut Vec<u8>, bytes: &[u8]) {
    for byte in bytes {
        match byte {
            b'\\' => escaped.extend_from_slice(b"\\\\"),
            b'\n' => escaped.extend_from_slice(b"\\n"),
            b'\r' => escaped.extend_from_slice(b"\\r"),
            b'\t' => escaped.extend_from_slice(b"\\t"),
            0x00..=0x1f | 0x7f => append_hex_escape(escaped, *byte),
            _ => escaped.push(*byte),
        }
    }
}

fn append_hex_escape(escaped: &mut Vec<u8>, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    escaped.extend_from_slice(&[
        b'\\',
        b'x',
        HEX[(byte >> 4) as usize],
        HEX[(byte & 0xf) as usize],
    ]);
}

fn error(callable: Option<CallableId>, message: &str) -> BackendError {
    BackendError::new(Target::X86_64SysV, callable, message)
}
