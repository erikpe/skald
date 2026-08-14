use super::*;
use crate::{
    identity::{
        BindingId, CallableId, ClassId, ClassTemplateId, CopyAssignmentId, CopyConstructorId,
        DestructorId, FunctionId, InitializerId, InterfaceId, InterfaceRequirementId, LocalId,
        MethodId, ParameterId, TypeParameterId,
    },
    lexer::lex,
    literal::{IntegerRadix, NumericLiteralKind},
    source::SourceDatabase,
    syntax::{self, parse, Statement},
    test_support::resolve_source,
};

use crate::resolve::dump_resolved;

fn resolve_text(text: &str) -> ResolveOutput {
    resolve_source(text)
}

fn local_initializer(statement: &ResolvedStatement) -> &ResolvedExpression {
    let ResolvedStatement::Local(local) = statement else {
        panic!("expected local declaration");
    };
    &local.initializer
}

fn return_value(statement: &ResolvedStatement) -> &ResolvedExpression {
    let ResolvedStatement::Return(statement) = statement else {
        panic!("expected return statement");
    };
    statement.value.as_ref().expect("expected a return value")
}

mod alias_parameters;
mod arrays;
mod bindings;
mod bitwise_operators;
mod comparisons;
mod control_flow;
mod cyclic_imports;
mod declarations;
mod diagnostics;
mod dumps;
mod expressions;
mod external_links;
mod generic_classes;
mod generic_object_model;
mod integer_division;
mod interfaces;
mod intrinsics;
mod modules;
mod objects;
mod optional_values;
mod primitive_binding_assignment;
mod primitive_casts;
mod produced_receivers;
mod shared_ownership;
mod shifts;
mod short_circuit_boolean;
mod static_fields;
mod static_methods;
mod strings;
mod structural_indexing;
mod type_operations;
mod vectors;
mod while_loops;
