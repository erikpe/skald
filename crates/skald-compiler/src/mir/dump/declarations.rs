//! Rendering for MIR declarations and class lifecycle capabilities.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::super::model::*;
use super::body::dump_executable_body;
use super::value::dump_copy_operation;

pub(super) fn dump_class(output: &mut String, class: &MirClassDeclaration) {
    let _ = write!(output, "    Class {} module {} ", class.id, class.module);
    write_quoted(output, &class.name);
    write_span(output, class.span);
    output.push('\n');
    if let Some(base) = class.direct_base {
        let _ = write!(output, "      DirectBase {}", base.class);
        write_span(output, base.span);
        output.push('\n');
    }
    for conformance in &class.conformances {
        let _ = writeln!(output, "      Conformance {}", conformance.interface);
        for implementation in &conformance.implementations {
            let _ = writeln!(
                output,
                "        {} -> {}",
                implementation.requirement, implementation.method
            );
        }
    }
    for field in &class.fields {
        let _ = write!(output, "      Field {} ", field.id);
        if field.cell_span.is_some() {
            output.push_str("cell ");
        }
        if field.final_span.is_some() {
            output.push_str("final ");
        }
        write_quoted(output, &field.name);
        let _ = write!(output, " : {}", field.ty);
        write_span(output, field.span);
        output.push('\n');
        if let Some(span) = field.cell_span {
            let _ = writeln!(
                output,
                "        Cell @{}..{}",
                span.range().start(),
                span.range().end()
            );
        }
        if let Some(span) = field.final_span {
            let _ = writeln!(
                output,
                "        Final @{}..{}",
                span.range().start(),
                span.range().end()
            );
        }
    }
    for field in &class.static_fields {
        let _ = write!(output, "      StaticField {} ", field.id);
        if field.final_span.is_some() {
            output.push_str("final ");
        }
        write_quoted(output, &field.name);
        let _ = write!(output, " : {} {}", field.ty, field.initialization);
        write_span(output, field.span);
        output.push('\n');
        if let Some(span) = field.final_span {
            let _ = writeln!(
                output,
                "        Final @{}..{}",
                span.range().start(),
                span.range().end()
            );
        }
    }
    for initializer in &class.initializers {
        let _ = write!(output, "      Initializer {}(", initializer.id);
        dump_parameters(output, &initializer.parameters);
        output.push(')');
        write_span(output, initializer.span);
        output.push('\n');
    }
    dump_copy_capability(output, "CopyConstructor", &class.copy_constructor);
    dump_copy_capability(output, "CopyAssignment", &class.copy_assignment);
    if !class.destruction.steps.is_empty() {
        output.push_str("      DestructionPlan\n");
        if let Some(destructor) = &class.destruction.destructor {
            let _ = write!(
                output,
                "        Destructor {} {}",
                destructor.id, destructor.receiver_access
            );
            write_span(output, destructor.span);
            output.push('\n');
        }
        for step in &class.destruction.steps {
            match step {
                MirDestructionStep::UserBody(destructor) => {
                    let _ = writeln!(output, "        UserBody {destructor}");
                }
                MirDestructionStep::Field(field) => {
                    let _ = writeln!(output, "        Field {field}");
                }
                MirDestructionStep::SharedField(field) => {
                    let _ = writeln!(output, "        SharedField {field}");
                }
                MirDestructionStep::OptionalSharedField(field) => {
                    let _ = writeln!(output, "        OptionalSharedField {field}");
                }
                MirDestructionStep::OptionalClassField(field) => {
                    let _ = writeln!(output, "        OptionalClassField {field}");
                }
                MirDestructionStep::OptionalField { field, optional } => {
                    let _ = writeln!(output, "        OptionalField {field} : {optional}");
                }
                MirDestructionStep::ArrayField(field) => {
                    let _ = writeln!(output, "        ArrayField {field}");
                }
                MirDestructionStep::Base(base) => {
                    let _ = writeln!(output, "        Base {base}");
                }
            }
        }
    }
    for method in &class.methods {
        let _ = write!(output, "      Method {} ", method.id);
        write_quoted(output, &method.name);
        match method.kind {
            MirMethodKind::Instance { receiver_access } => {
                let _ = write!(output, " {receiver_access} (");
            }
            MirMethodKind::Static => output.push_str(" static ("),
        }
        dump_parameters(output, &method.parameters);
        let _ = write!(output, ") -> {}", method.return_type);
        write_span(output, method.span);
        output.push('\n');
    }
}

fn dump_copy_capability<I: Copy + std::fmt::Display>(
    output: &mut String,
    label: &str,
    capability: &MirCopyCapability<I>,
) {
    let _ = writeln!(output, "      {label}");
    match capability {
        MirCopyCapability::User(copy) => {
            let _ = writeln!(output, "        User {}", copy.operation);
            dump_base_copy(output, copy.base);
        }
        MirCopyCapability::Synthesized(copy) => {
            let _ = writeln!(output, "        Synthesized {}", copy.class);
            dump_base_copy(output, copy.base);
            for field in &copy.final_fields {
                let _ = writeln!(output, "          FinalField {field}");
            }
            for field in &copy.fields {
                match field {
                    MirSynthesizedFieldCopy::Scalar { field } => {
                        let _ = writeln!(output, "          Scalar {field}");
                    }
                    MirSynthesizedFieldCopy::OptionalPrimitive { field, payload } => {
                        let _ =
                            writeln!(output, "          OptionalPrimitive {field} : {payload}?");
                    }
                    MirSynthesizedFieldCopy::Shared { field } => {
                        let _ = writeln!(output, "          Shared {field}");
                    }
                    MirSynthesizedFieldCopy::OptionalShared { field, target } => {
                        let _ = writeln!(
                            output,
                            "          OptionalShared {field} : shared? {target}"
                        );
                    }
                    MirSynthesizedFieldCopy::Optional { field, optional } => {
                        let _ = writeln!(output, "          Optional {field} : {optional}");
                    }
                    MirSynthesizedFieldCopy::OptionalClass {
                        field,
                        class,
                        operation,
                    } => {
                        let _ = write!(
                            output,
                            "          OptionalClass {field} : class {class}? via "
                        );
                        dump_copy_operation(output, *operation);
                        output.push('\n');
                    }
                    MirSynthesizedFieldCopy::Class { field, operation } => {
                        let _ = write!(output, "          Class {field} via ");
                        dump_copy_operation(output, *operation);
                        output.push('\n');
                    }
                    MirSynthesizedFieldCopy::Array { field, array } => {
                        let _ = writeln!(output, "          Array {field} : {array}");
                    }
                }
            }
        }
        MirCopyCapability::Unavailable => output.push_str("        Unavailable\n"),
    }
}

fn dump_base_copy<I: Copy + std::fmt::Display>(output: &mut String, copy: Option<MirBaseCopy<I>>) {
    if let Some(copy) = copy {
        let _ = write!(output, "          Base {} via ", copy.base);
        dump_copy_operation(output, copy.operation);
        output.push('\n');
    }
}

pub(super) fn dump_parameters(output: &mut String, parameters: &[MirParameter]) {
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        match parameter.mode {
            MirParameterMode::Value => {}
            MirParameterMode::ReadOnlyAlias => output.push_str("ref "),
            MirParameterMode::MutableAlias => output.push_str("mut ref "),
        }
        let _ = write!(output, "{}", parameter.ty);
    }
}

pub(super) fn dump_declaration(output: &mut String, declaration: &MirFunctionDeclaration) {
    let _ = write!(
        output,
        "    Declaration {} module {} ",
        declaration.id, declaration.module
    );
    write_quoted(output, &declaration.name);
    match &declaration.linkage {
        MirFunctionLinkage::Internal => output.push_str(" internal"),
        MirFunctionLinkage::External { link } => {
            let _ = write!(output, " external {link}");
        }
        MirFunctionLinkage::Intrinsic { intrinsic } => {
            let _ = write!(output, " intrinsic {intrinsic:?}");
        }
    }
    write_span(output, declaration.span);
    output.push('\n');
    output.push_str("      Signature (");
    dump_parameters(output, &declaration.parameters);
    let _ = writeln!(output, ") -> {}", declaration.return_type);
}

pub(super) fn dump_definition(output: &mut String, function: &MirFunctionDefinition) {
    let _ = write!(output, "    Definition {}", function.function);
    dump_executable_body(output, function.into());
}

pub(super) fn dump_member_definition(output: &mut String, function: &MirMemberDefinition) {
    let _ = write!(output, "    MemberDefinition {}", function.callable);
    dump_executable_body(output, function.into());
}
