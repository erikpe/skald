//! Closed-world retention for target-generated functions and data.
//!
//! MIR remains complete for verification and deterministic dumps. Once it has
//! been lowered, however, the machine program contains explicit symbols for
//! every dependency that can survive into assembly. Walking those references
//! lets textual emission omit declarations that were needed only by an erased
//! source-level operation, as well as ordinary unreachable source artifacts.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::machine::{AssemblyProgram, Instruction};

pub(super) fn retain_reachable(program: &mut AssemblyProgram) {
    let graph = dependency_graph(program);
    let mut pending: VecDeque<String> = program
        .functions
        .iter()
        .filter(|function| function.exported)
        .map(|function| function.symbol.clone())
        .collect();
    let mut reachable = BTreeSet::new();

    while let Some(symbol) = pending.pop_front() {
        if !reachable.insert(symbol.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&symbol) {
            pending.extend(dependencies.iter().cloned());
        }
    }

    program
        .functions
        .retain(|function| reachable.contains(&function.symbol));
    program
        .static_slots
        .retain(|slot| reachable.contains(&slot.symbol));
    program
        .dispatch_tables
        .retain(|table| reachable.contains(&table.symbol));
    program
        .literal_backings
        .retain(|backing| reachable.contains(&backing.symbol));
    program
        .panic_messages
        .retain(|message| reachable.contains(&message.symbol));
    program
        .runtime_trace
        .strings
        .retain(|string| reachable.contains(&string.symbol));
    program
        .runtime_trace
        .contexts
        .retain(|context| reachable.contains(&context.symbol));
    program
        .runtime_trace
        .locations
        .retain(|location| reachable.contains(&location.symbol));
}

fn dependency_graph(program: &AssemblyProgram) -> BTreeMap<String, Vec<String>> {
    let mut graph = BTreeMap::new();

    for function in &program.functions {
        insert_artifact(
            &mut graph,
            &function.symbol,
            function.instructions.iter().filter_map(instruction_symbol),
        );
    }
    for slot in &program.static_slots {
        insert_artifact(&mut graph, &slot.symbol, std::iter::empty());
    }
    for table in &program.dispatch_tables {
        insert_artifact(
            &mut graph,
            &table.symbol,
            table.entries.iter().filter_map(Option::as_deref),
        );
    }
    for backing in &program.literal_backings {
        insert_artifact(
            &mut graph,
            &backing.symbol,
            std::iter::once(backing.metadata_symbol.as_str()),
        );
    }
    for message in &program.panic_messages {
        insert_artifact(&mut graph, &message.symbol, std::iter::empty());
    }
    for string in &program.runtime_trace.strings {
        insert_artifact(&mut graph, &string.symbol, std::iter::empty());
    }
    for context in &program.runtime_trace.contexts {
        insert_artifact(
            &mut graph,
            &context.symbol,
            [context.name_symbol.as_str(), context.path_symbol.as_str()],
        );
    }
    for location in &program.runtime_trace.locations {
        insert_artifact(
            &mut graph,
            &location.symbol,
            std::iter::once(location.context_symbol.as_str()),
        );
    }

    graph
}

fn insert_artifact<'a>(
    graph: &mut BTreeMap<String, Vec<String>>,
    symbol: &str,
    dependencies: impl IntoIterator<Item = &'a str>,
) {
    let previous = graph.insert(
        symbol.to_owned(),
        dependencies.into_iter().map(str::to_owned).collect(),
    );
    debug_assert!(
        previous.is_none(),
        "duplicate assembly artifact symbol `{symbol}`"
    );
}

pub(super) fn instruction_symbol(instruction: &Instruction) -> Option<&str> {
    match instruction {
        Instruction::LoadSymbolAddress { symbol, .. }
        | Instruction::LoadRuntimeTraceLocationAddress { symbol, .. }
        | Instruction::Call(symbol) => Some(symbol),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::x86_64_sysv::machine::{
        AssemblyDispatchTable, AssemblyFunction, AssemblyLiteralBacking, AssemblyPanicMessage,
        AssemblyRuntimeTraceMetadata, AssemblyStaticSlot, AssemblyTraceByteString,
        AssemblyTraceContext, AssemblyTraceLocation, Register,
    };

    #[test]
    fn prunes_unreachable_artifacts_from_every_section() {
        let mut program = fixture_program(vec![function("main", true, vec![])]);
        program.functions.push(function("dead", false, vec![]));

        retain_reachable(&mut program);

        assert_eq!(symbols(&program.functions, |item| &item.symbol), ["main"]);
        assert!(program.static_slots.is_empty());
        assert!(program.dispatch_tables.is_empty());
        assert!(program.literal_backings.is_empty());
        assert!(program.panic_messages.is_empty());
        assert!(program.runtime_trace.is_empty());
    }

    #[test]
    fn retains_transitive_function_and_data_dependencies() {
        let mut program = fixture_program(vec![
            function("main", true, vec![Instruction::Call("worker".to_owned())]),
            function(
                "worker",
                false,
                vec![
                    load("slot"),
                    load("table"),
                    load("literal"),
                    load("panic"),
                    Instruction::LoadRuntimeTraceLocationAddress {
                        symbol: "location".to_owned(),
                        destination: Register::Rax,
                    },
                ],
            ),
            function("witness", false, vec![]),
            function("finalizer", false, vec![]),
        ]);

        retain_reachable(&mut program);

        assert_eq!(program.functions.len(), 4);
        assert_eq!(program.static_slots.len(), 1);
        assert_eq!(program.dispatch_tables.len(), 2);
        assert_eq!(program.literal_backings.len(), 1);
        assert_eq!(program.panic_messages.len(), 1);
        assert_eq!(program.runtime_trace.strings.len(), 2);
        assert_eq!(program.runtime_trace.contexts.len(), 1);
        assert_eq!(program.runtime_trace.locations.len(), 1);
    }

    fn fixture_program(functions: Vec<AssemblyFunction>) -> AssemblyProgram {
        AssemblyProgram {
            functions,
            static_slots: vec![AssemblyStaticSlot {
                field: crate::identity::StaticFieldId::new(crate::identity::ClassId::new(0), 0),
                symbol: "slot".to_owned(),
                size: 8,
                alignment_power: 3,
            }],
            dispatch_tables: vec![
                AssemblyDispatchTable {
                    symbol: "table".to_owned(),
                    entries: vec![Some("witness".to_owned()), Some("finalizer".to_owned())],
                },
                AssemblyDispatchTable {
                    symbol: "metadata".to_owned(),
                    entries: vec![Some("finalizer".to_owned())],
                },
            ],
            literal_backings: vec![AssemblyLiteralBacking {
                symbol: "literal".to_owned(),
                metadata_symbol: "metadata".to_owned(),
                bytes: vec![1],
            }],
            panic_messages: vec![AssemblyPanicMessage {
                symbol: "panic".to_owned(),
                bytes: b"panic",
            }],
            runtime_trace: AssemblyRuntimeTraceMetadata {
                strings: vec![trace_string("name"), trace_string("path")],
                contexts: vec![AssemblyTraceContext {
                    symbol: "context".to_owned(),
                    name_symbol: "name".to_owned(),
                    name_length: 4,
                    path_symbol: "path".to_owned(),
                    path_length: 4,
                }],
                locations: vec![AssemblyTraceLocation {
                    symbol: "location".to_owned(),
                    context_symbol: "context".to_owned(),
                    line: 1,
                    column: 1,
                }],
            },
        }
    }

    fn function(symbol: &str, exported: bool, instructions: Vec<Instruction>) -> AssemblyFunction {
        AssemblyFunction {
            symbol: symbol.to_owned(),
            exported,
            instructions,
        }
    }

    fn load(symbol: &str) -> Instruction {
        Instruction::LoadSymbolAddress {
            symbol: symbol.to_owned(),
            destination: Register::Rax,
        }
    }

    fn trace_string(symbol: &str) -> AssemblyTraceByteString {
        AssemblyTraceByteString {
            symbol: symbol.to_owned(),
            bytes: symbol.as_bytes().to_vec(),
        }
    }

    fn symbols<'a, T>(items: &'a [T], symbol: impl Fn(&'a T) -> &'a str) -> Vec<&'a str> {
        items.iter().map(symbol).collect()
    }
}
