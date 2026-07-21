//! Program-level validation and typed-HIR orchestration.

use crate::{
    diagnostics::{format_type_list, Diagnostic, Diagnostics},
    hir::{
        HirAccess, HirClassDeclaration, HirClassDeclarationTable, HirClassDefinition,
        HirClassDefinitionTable, HirCopyAssignmentDeclaration, HirDestructorDeclaration,
        HirFieldDeclaration, HirFunctionDeclaration, HirFunctionDeclarationTable,
        HirFunctionDefinitionTable, HirFunctionLinkage, HirInitializerDeclaration,
        HirMethodDeclaration, HirParameter, HirParameterMode, HirProgram, Type,
    },
    identity::FunctionId,
    resolve::{
        ResolvedClassDeclaration, ResolvedFunctionDeclaration, ResolvedFunctionLinkage,
        ResolvedParameter, ResolvedParameterBindingMode, ResolvedProgram, ResolvedReceiverAccess,
        ResolvedType, ResolvedTypeKind,
    },
    source::Span,
};

use super::{
    capabilities::CopyCapabilities,
    containment::validate_containment,
    function::{CallableChecker, MemberBodyKind, MemberCheckContext, ReceiverContext},
};

const EXTERNAL_PARAMETER_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool"];
const EXTERNAL_RESULT_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool", "unit"];
const DESTRUCTOR_RECEIVER_ACCESS: HirAccess = HirAccess::Mutable;

pub const MISSING_ENTRY_POINT: &str = "TYP001";
pub const INVALID_ENTRY_POINT: &str = "TYP002";
pub const INTEGER_LITERAL_OUT_OF_RANGE: &str = "TYP003";
pub const WRONG_ARGUMENT_COUNT: &str = "TYP004";
pub const TYPE_MISMATCH: &str = "TYP005";
pub const MISSING_RETURN: &str = "TYP006";
pub const INVALID_RETURN: &str = "TYP007";
pub const INVALID_CALL_STATEMENT: &str = "TYP008";
pub const INVALID_EXTERNAL_DECLARATION: &str = "TYP009";
pub const U64_LITERAL_OUT_OF_RANGE: &str = "TYP010";
pub const U8_LITERAL_OUT_OF_RANGE: &str = "TYP011";
pub const F64_LITERAL_OUT_OF_RANGE: &str = "TYP012";
pub const INVALID_OBJECT_DECLARATION: &str = "TYP013";
pub const INVALID_OBJECT_CONTEXT: &str = "TYP014";
pub const INVALID_CONSTRUCTION: &str = "TYP015";
pub const INVALID_INITIALIZER_BODY: &str = "TYP016";
pub const FIELD_INITIALIZATION: &str = "TYP017";
pub const READ_ONLY_RECEIVER: &str = "TYP018";
pub const INVALID_ALIAS_PARAMETER: &str = "TYP019";
pub const INVALID_ALIAS_ARGUMENT: &str = "TYP020";
pub const INSUFFICIENT_ALIAS_ACCESS: &str = "TYP021";
pub const COPY_OPERATION_UNAVAILABLE: &str = "TYP023";

#[derive(Debug)]
pub struct TypeCheckOutput {
    /// Present only when the entire resolved program type-checks successfully.
    pub hir: Option<HirProgram>,
    pub diagnostics: Diagnostics,
}

impl TypeCheckOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

pub fn type_check(program: &ResolvedProgram) -> TypeCheckOutput {
    let mut diagnostics = Diagnostics::new();
    check_internal_function_parameters(program, &mut diagnostics);
    check_external_declarations(program, &mut diagnostics);
    let entry_function = check_entry_point(program, &mut diagnostics);
    validate_containment(program, &mut diagnostics);
    let copy_capabilities = CopyCapabilities::compute(program);
    let classes = lower_class_declarations(program, &copy_capabilities, &mut diagnostics);
    let declarations = program.declarations.iter().map(lower_declaration).collect();
    let definitions = program
        .declarations
        .iter()
        .map(|declaration| {
            program.definitions.get(declaration.id).map(|definition| {
                CallableChecker::new(
                    program,
                    &copy_capabilities,
                    declaration,
                    definition,
                    &mut diagnostics,
                )
                .check()
            })
        })
        .collect();
    let class_definitions = check_class_definitions(program, &copy_capabilities, &mut diagnostics);

    let hir = if diagnostics.has_errors() {
        None
    } else {
        Some(HirProgram {
            classes: HirClassDeclarationTable::new(classes),
            class_definitions: HirClassDefinitionTable::new(class_definitions),
            declarations: HirFunctionDeclarationTable::new(declarations),
            definitions: HirFunctionDefinitionTable::new(definitions),
            entry_function: entry_function.expect("valid program must have an entry function"),
            span: program.span,
        })
    };

    TypeCheckOutput { hir, diagnostics }
}

fn check_internal_function_parameters(program: &ResolvedProgram, diagnostics: &mut Diagnostics) {
    for declaration in program.declarations.iter() {
        if matches!(declaration.linkage, ResolvedFunctionLinkage::Internal) {
            validate_parameters(&declaration.parameters, diagnostics, "function");
        }
    }
}

fn lower_class_declarations(
    program: &ResolvedProgram,
    copy_capabilities: &CopyCapabilities,
    diagnostics: &mut Diagnostics,
) -> Vec<HirClassDeclaration> {
    program
        .classes
        .iter()
        .filter_map(|class| lower_class_declaration(class, copy_capabilities, diagnostics))
        .collect()
}

fn lower_class_declaration(
    class: &ResolvedClassDeclaration,
    copy_capabilities: &CopyCapabilities,
    diagnostics: &mut Diagnostics,
) -> Option<HirClassDeclaration> {
    let mut valid = true;
    let fields = class
        .fields
        .iter()
        .map(|field| {
            let ty = lower_type(&field.type_syntax);
            if ty == Type::Unit {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!("field `{}` cannot have type `unit`", field.name),
                    )
                    .with_primary_label(field.type_syntax.span, "fields require storage"),
                );
                valid = false;
            }
            HirFieldDeclaration {
                id: field.id,
                name: field.name.clone(),
                name_span: field.name_span,
                ty,
                span: field.span,
            }
        })
        .collect();
    let Some(initializer) = &class.initializer else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_OBJECT_DECLARATION,
                format!("class `{}` requires an explicit initializer", class.name),
            )
            .with_primary_label(
                class.name_span,
                "add `init() {}` even when the class is empty",
            ),
        );
        return None;
    };
    valid &= validate_parameters(&initializer.parameters, diagnostics, "initializer");
    let initializer = HirInitializerDeclaration {
        id: initializer.id,
        parameters: initializer.parameters.iter().map(lower_parameter).collect(),
        span: initializer.span,
    };
    let copy_constructor_declaration =
        class
            .copy_constructor_declaration
            .as_ref()
            .map(|copy| HirInitializerDeclaration {
                id: copy.id,
                parameters: copy.parameters.iter().map(lower_parameter).collect(),
                span: copy.span,
            });
    let copy_assignment_declaration =
        class
            .copy_assignment_declaration
            .as_ref()
            .map(|copy| HirCopyAssignmentDeclaration {
                id: copy.id,
                parameter: lower_parameter(&copy.parameter),
                span: copy.span,
            });
    let destructor = class
        .destructor
        .as_ref()
        .map(|destructor| HirDestructorDeclaration {
            id: destructor.id,
            receiver_access: DESTRUCTOR_RECEIVER_ACCESS,
            span: destructor.span,
        });
    let methods = class
        .methods
        .iter()
        .map(|method| {
            valid &= validate_parameters(&method.parameters, diagnostics, "method");
            let return_type = lower_type(&method.return_type);
            if !is_payload_primitive(return_type) && return_type != Type::Unit {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!("method `{}` has an unavailable result type", method.name),
                    )
                    .with_primary_label(
                        method.return_type.span,
                        "expected a primitive type or `unit`",
                    ),
                );
                valid = false;
            }
            HirMethodDeclaration {
                id: method.id,
                name: method.name.clone(),
                name_span: method.name_span,
                receiver_access: lower_receiver_access(method.receiver_access),
                parameters: method.parameters.iter().map(lower_parameter).collect(),
                return_type,
                span: method.span,
            }
        })
        .collect();
    valid.then_some(HirClassDeclaration {
        id: class.id,
        name: class.name.clone(),
        name_span: class.name_span,
        fields,
        initializer,
        copy_constructor_declaration,
        copy_constructor: copy_capabilities.constructor(class.id).clone(),
        copy_assignment_declaration,
        copy_assignment: copy_capabilities.assignment(class.id).clone(),
        destructor,
        methods,
        span: class.span,
    })
}

fn check_class_definitions(
    program: &ResolvedProgram,
    copy_capabilities: &CopyCapabilities,
    diagnostics: &mut Diagnostics,
) -> Vec<HirClassDefinition> {
    program
        .classes
        .iter()
        .filter_map(|class| {
            let initializer = class.initializer.as_ref()?;
            let definition = program.class_definitions.get(class.id)?;
            let initializer_definition = definition.initializer.as_ref()?;
            let initializer_body = CallableChecker::new_member(
                program,
                copy_capabilities,
                MemberCheckContext {
                    callable: initializer.id.into(),
                    parameters: &initializer.parameters,
                    definition: initializer_definition,
                    return_type: Type::Unit,
                    receiver: ReceiverContext {
                        class: class.id,
                        access: HirAccess::Mutable,
                        body_kind: MemberBodyKind::OrdinaryInitializer,
                    },
                    callable_name: format!("initializer for class `{}`", class.name),
                },
                diagnostics,
            )
            .check_member();
            let copy_constructor = class.copy_constructor_declaration.as_ref().map(|copy| {
                let body = definition
                    .copy_constructor
                    .as_ref()
                    .expect("resolved copy-constructor declaration must have a body");
                CallableChecker::new_member(
                    program,
                    copy_capabilities,
                    MemberCheckContext {
                        callable: copy.id.into(),
                        parameters: &copy.parameters,
                        definition: body,
                        return_type: Type::Unit,
                        receiver: ReceiverContext {
                            class: class.id,
                            access: HirAccess::Mutable,
                            body_kind: MemberBodyKind::CopyConstructor,
                        },
                        callable_name: format!("copy constructor for class `{}`", class.name),
                    },
                    diagnostics,
                )
                .check_member()
            });
            let copy_assignment = class.copy_assignment_declaration.as_ref().map(|copy| {
                let body = definition
                    .copy_assignment
                    .as_ref()
                    .expect("resolved copy-assignment declaration must have a body");
                CallableChecker::new_member(
                    program,
                    copy_capabilities,
                    MemberCheckContext {
                        callable: copy.id.into(),
                        parameters: std::slice::from_ref(&copy.parameter),
                        definition: body,
                        return_type: Type::Unit,
                        receiver: ReceiverContext {
                            class: class.id,
                            access: HirAccess::Mutable,
                            body_kind: MemberBodyKind::CopyAssignment,
                        },
                        callable_name: format!("copy assignment for class `{}`", class.name),
                    },
                    diagnostics,
                )
                .check_member()
            });
            let destructor = class.destructor.as_ref().map(|destructor| {
                let body = definition
                    .destructor
                    .as_ref()
                    .expect("resolved destructor declaration must have a body");
                CallableChecker::new_member(
                    program,
                    copy_capabilities,
                    MemberCheckContext {
                        callable: destructor.id.into(),
                        parameters: &[],
                        definition: body,
                        return_type: Type::Unit,
                        receiver: ReceiverContext {
                            class: class.id,
                            access: DESTRUCTOR_RECEIVER_ACCESS,
                            body_kind: MemberBodyKind::MethodOrDestructor,
                        },
                        callable_name: format!("destructor for class `{}`", class.name),
                    },
                    diagnostics,
                )
                .check_member()
            });
            let methods = class
                .methods
                .iter()
                .zip(&definition.methods)
                .map(|(method, body)| {
                    CallableChecker::new_member(
                        program,
                        copy_capabilities,
                        MemberCheckContext {
                            callable: method.id.into(),
                            parameters: &method.parameters,
                            definition: body,
                            return_type: lower_type(&method.return_type),
                            receiver: ReceiverContext {
                                class: class.id,
                                access: lower_receiver_access(method.receiver_access),
                                body_kind: MemberBodyKind::MethodOrDestructor,
                            },
                            callable_name: format!("method `{}`", method.name),
                        },
                        diagnostics,
                    )
                    .check_member()
                })
                .collect();
            Some(HirClassDefinition {
                class: class.id,
                initializer: initializer_body,
                copy_constructor,
                copy_assignment,
                destructor,
                methods,
                span: definition.span,
            })
        })
        .collect()
}

fn validate_parameters(
    parameters: &[ResolvedParameter],
    diagnostics: &mut Diagnostics,
    owner: &'static str,
) -> bool {
    let mut valid = true;
    for parameter in parameters {
        let ty = lower_type(&parameter.type_syntax);
        match parameter.binding_mode {
            ResolvedParameterBindingMode::Value if ty == Type::Unit => {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!(
                            "{owner} parameter `{}` requires a stored value type",
                            parameter.name
                        ),
                    )
                    .with_primary_label(
                        parameter.type_syntax.span,
                        "`unit` value parameters are unavailable",
                    ),
                );
                valid = false;
            }
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
            | ResolvedParameterBindingMode::MutableAlias { .. }
                if !matches!(ty, Type::Class(_)) =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_ALIAS_PARAMETER,
                        format!(
                            "{owner} alias parameter `{}` must name a class",
                            parameter.name
                        ),
                    )
                    .with_primary_label(
                        parameter.type_syntax.span,
                        "primitive and `unit` aliases are unavailable",
                    ),
                );
                valid = false;
            }
            _ => {}
        }
    }
    valid
}

fn lower_parameter(parameter: &ResolvedParameter) -> HirParameter {
    HirParameter {
        id: parameter.id,
        mode: lower_parameter_mode(parameter.binding_mode),
        name: parameter.name.clone(),
        name_span: parameter.name_span,
        ty: lower_type(&parameter.type_syntax),
        span: parameter.span,
    }
}

const fn is_payload_primitive(ty: Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
    )
}

pub(super) const fn lower_parameter_mode(mode: ResolvedParameterBindingMode) -> HirParameterMode {
    match mode {
        ResolvedParameterBindingMode::Value => HirParameterMode::Value,
        ResolvedParameterBindingMode::ReadOnlyAlias { .. } => HirParameterMode::ReadOnlyAlias,
        ResolvedParameterBindingMode::MutableAlias { .. } => HirParameterMode::MutableAlias,
    }
}

const fn lower_receiver_access(access: ResolvedReceiverAccess) -> HirAccess {
    match access {
        ResolvedReceiverAccess::ReadOnly => HirAccess::ReadOnly,
        ResolvedReceiverAccess::Mutable => HirAccess::Mutable,
    }
}

fn check_entry_point(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> Option<FunctionId> {
    let Some(entry_id) = program.entry_function else {
        let start = program.span.range().start();
        diagnostics.push(
            Diagnostic::error(MISSING_ENTRY_POINT, "missing entry function `main`")
                .with_primary_label(
                    Span::empty(program.span.source_id(), start),
                    "define `fn main() -> i64` in this file",
                ),
        );
        return None;
    };
    let entry = program
        .declarations
        .get(entry_id)
        .expect("resolved entry ID must exist in the declaration table");
    let return_type = lower_type(&entry.return_type);

    if !matches!(entry.linkage, ResolvedFunctionLinkage::Internal)
        || program.definitions.get(entry_id).is_none()
    {
        diagnostics.push(
            Diagnostic::error(
                INVALID_ENTRY_POINT,
                "entry function must have signature `fn main() -> i64`",
            )
            .with_primary_label(
                entry.name_span,
                "an external declaration cannot be the entry point",
            )
            .with_note("define `fn main() -> i64` with a Skald function body"),
        );
        return None;
    }

    if !entry.parameters.is_empty() || return_type != Type::I64 {
        diagnostics.push(
            Diagnostic::error(
                INVALID_ENTRY_POINT,
                "entry function must have signature `fn main() -> i64`",
            )
            .with_primary_label(entry.name_span, "invalid entry signature")
            .with_note(format!(
                "found {} parameter{} and return type `{}`",
                entry.parameters.len(),
                if entry.parameters.len() == 1 { "" } else { "s" },
                return_type.name()
            )),
        );
        return None;
    }

    Some(entry_id)
}

fn check_external_declarations(program: &ResolvedProgram, diagnostics: &mut Diagnostics) {
    for declaration in program.declarations.iter() {
        let ResolvedFunctionLinkage::External { symbol } = &declaration.linkage else {
            continue;
        };
        if let Some(parameter) = declaration
            .parameters
            .iter()
            .find(|parameter| parameter.binding_mode != ResolvedParameterBindingMode::Value)
        {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_EXTERNAL_DECLARATION,
                    format!(
                        "external function `{}` cannot declare alias parameters",
                        declaration.name
                    ),
                )
                .with_primary_label(parameter.span, "aliases have no supported C ABI yet")
                .with_note("external parameters must be passed by value"),
            );
            continue;
        }
        let has_valid_parameters = declaration.parameters.iter().all(|parameter| {
            matches!(
                lower_type(&parameter.type_syntax),
                Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
            )
        });
        let has_valid_return = matches!(
            lower_type(&declaration.return_type),
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool | Type::Unit
        );
        if !has_valid_parameters || !has_valid_return || symbol != &declaration.name {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_EXTERNAL_DECLARATION,
                    format!(
                        "external function `{}` has an unsupported signature",
                        declaration.name
                    ),
                )
                .with_primary_label(
                    declaration.span,
                    format!(
                        "expected by-value {} parameters and a result of type {}",
                        format_type_list(EXTERNAL_PARAMETER_TYPE_NAMES),
                        format_type_list(EXTERNAL_RESULT_TYPE_NAMES)
                    ),
                )
                .with_note("the source function name must also be its exact linker symbol"),
            );
        }
    }
}

fn lower_declaration(function: &ResolvedFunctionDeclaration) -> HirFunctionDeclaration {
    let parameters = function.parameters.iter().map(lower_parameter).collect();

    HirFunctionDeclaration {
        id: function.id,
        name: function.name.clone(),
        name_span: function.name_span,
        parameters,
        return_type: lower_type(&function.return_type),
        linkage: match &function.linkage {
            ResolvedFunctionLinkage::Internal => HirFunctionLinkage::Internal,
            ResolvedFunctionLinkage::External { symbol } => HirFunctionLinkage::External {
                symbol: symbol.clone(),
            },
        },
        span: function.span,
    }
}

pub(super) fn lower_type(type_syntax: &ResolvedType) -> Type {
    match type_syntax.kind {
        ResolvedTypeKind::I64 => Type::I64,
        ResolvedTypeKind::U64 => Type::U64,
        ResolvedTypeKind::U8 => Type::U8,
        ResolvedTypeKind::F64 => Type::F64,
        ResolvedTypeKind::Bool => Type::Bool,
        ResolvedTypeKind::Unit => Type::Unit,
        ResolvedTypeKind::Class(class) => Type::Class(class),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{identity::ClassId, test_support::resolve_source};

    #[test]
    fn lowers_named_fields_to_canonical_class_types() {
        let resolved = resolve_source(concat!(
            "class Outer { child: Inner; init() {} }\n",
            "class Inner { init() {} }\n",
            "fn main() -> i64 { return 0; }\n",
        ));
        assert!(resolved.diagnostics.is_empty());

        let mut diagnostics = Diagnostics::new();
        let copy_capabilities = CopyCapabilities::compute(&resolved.program);
        let outer = lower_class_declaration(
            resolved.program.classes.get(ClassId::new(0)).unwrap(),
            &copy_capabilities,
            &mut diagnostics,
        )
        .expect("class-typed fields should lower to HIR declarations");

        assert!(diagnostics.is_empty());
        assert_eq!(outer.fields[0].ty, Type::Class(ClassId::new(1)));
    }
}
