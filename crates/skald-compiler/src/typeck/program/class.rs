//! Class declaration lowering and member-body checking.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        HirAccess, HirClassDeclaration, HirClassDefinition, HirCopyAssignmentDeclaration,
        HirCopyConstructorDeclaration, HirDestructionPlan, HirDestructionStep,
        HirDestructorDeclaration, HirDirectBase, HirFieldDeclaration, HirInitializerDeclaration,
        HirInterfaceConformance, HirMemberDefinition, HirMethodDeclaration, HirMethodDispatch,
        Type,
    },
    identity::CallableId,
    resolve::{
        ResolvedClassDeclaration, ResolvedClassDefinition, ResolvedMemberDefinition,
        ResolvedMethodDispatch, ResolvedParameter, ResolvedProgram, ResolvedReceiverAccess,
    },
};

use super::{lower_parameter, lower_type, validate_parameters, INVALID_OBJECT_DECLARATION};
use crate::typeck::{
    capabilities::CopyCapabilities,
    function::{CallableChecker, MemberBodyKind, MemberCheckContext, ReceiverContext},
};

const DESTRUCTOR_RECEIVER_ACCESS: HirAccess = HirAccess::Mutable;

pub(super) fn lower_class_declarations(
    program: &ResolvedProgram,
    copy_capabilities: &CopyCapabilities,
    conformances: &[Vec<HirInterfaceConformance>],
    diagnostics: &mut Diagnostics,
) -> Vec<HirClassDeclaration> {
    program
        .classes
        .iter()
        .filter_map(|class| {
            lower_class_declaration(
                class,
                copy_capabilities,
                conformances[class.id.index()].clone(),
                diagnostics,
            )
        })
        .collect()
}

fn lower_class_declaration(
    class: &ResolvedClassDeclaration,
    copy_capabilities: &CopyCapabilities,
    conformances: Vec<HirInterfaceConformance>,
    diagnostics: &mut Diagnostics,
) -> Option<HirClassDeclaration> {
    let mut valid = true;
    let fields: Vec<_> = class
        .fields
        .iter()
        .map(|field| {
            let ty = lower_type(&field.type_syntax);
            if matches!(ty, Type::Unit | Type::Obj | Type::Interface(_)) {
                let name = ty.name();
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!("field `{}` cannot have type `{name}`", field.name),
                    )
                    .with_primary_label(
                        field.type_syntax.span,
                        "`Obj` and interfaces are non-owning views; `unit` has no storage",
                    ),
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
    if class.initializers.is_empty() {
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
    }
    let initializers = class
        .initializers
        .iter()
        .map(|initializer| {
            valid &= validate_parameters(&initializer.parameters, diagnostics, "initializer");
            HirInitializerDeclaration {
                id: initializer.id,
                parameters: initializer.parameters.iter().map(lower_parameter).collect(),
                span: initializer.span,
            }
        })
        .collect();
    let copy_constructor_declaration =
        class
            .copy_constructor_declaration
            .as_ref()
            .map(|copy| HirCopyConstructorDeclaration {
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
    let direct_base = class.direct_base.map(|base| HirDirectBase {
        class: base.class,
        span: base.span,
    });
    let owning_fields = fields
        .iter()
        .filter_map(|field| match field.ty {
            Type::Class(_) => Some(HirDestructionStep::Field(field.id)),
            Type::Shared(_) => Some(HirDestructionStep::SharedField(field.id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let destruction = HirDestructionPlan::new(
        destructor.as_ref().map(|destructor| destructor.id),
        &owning_fields,
        direct_base.as_ref().map(|base| base.class),
    );
    let methods = class
        .methods
        .iter()
        .map(|method| {
            valid &= validate_parameters(&method.parameters, diagnostics, "method");
            let return_type = lower_type(&method.return_type);
            if matches!(return_type, Type::Obj | Type::Interface(_)) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!("method `{}` cannot return a non-owning view", method.name),
                    )
                    .with_primary_label(
                        method.return_type.span,
                        "non-owning views cannot escape a call",
                    ),
                );
                valid = false;
            }
            HirMethodDeclaration {
                id: method.id,
                name: method.name.clone(),
                name_span: method.name_span,
                receiver_access: lower_receiver_access(method.receiver_access),
                dispatch: match method.dispatch {
                    ResolvedMethodDispatch::Direct => HirMethodDispatch::Direct,
                    ResolvedMethodDispatch::VirtualRoot { family, slot } => {
                        HirMethodDispatch::VirtualRoot { family, slot }
                    }
                    ResolvedMethodDispatch::Override {
                        family,
                        slot,
                        root,
                        overridden,
                    } => HirMethodDispatch::Override {
                        family,
                        slot,
                        root,
                        overridden,
                    },
                },
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
        direct_base,
        conformances,
        fields,
        initializers,
        copy_constructor_declaration,
        copy_constructor: copy_capabilities.constructor(class.id).clone(),
        copy_assignment_declaration,
        copy_assignment: copy_capabilities.assignment(class.id).clone(),
        destructor,
        destruction,
        methods,
        span: class.span,
    })
}

pub(super) fn check_class_definitions(
    program: &ResolvedProgram,
    copy_capabilities: &CopyCapabilities,
    diagnostics: &mut Diagnostics,
) -> Vec<HirClassDefinition> {
    program
        .classes
        .iter()
        .filter_map(|class| {
            let definition = program.class_definitions.get(class.id)?;
            ClassDefinitionChecker {
                program,
                copy_capabilities,
                class,
                definition,
                diagnostics,
            }
            .check()
        })
        .collect()
}

struct ClassDefinitionChecker<'program, 'diagnostics> {
    program: &'program ResolvedProgram,
    copy_capabilities: &'program CopyCapabilities,
    class: &'program ResolvedClassDeclaration,
    definition: &'program ResolvedClassDefinition,
    diagnostics: &'diagnostics mut Diagnostics,
}

impl ClassDefinitionChecker<'_, '_> {
    fn check(&mut self) -> Option<HirClassDefinition> {
        if self.class.initializers.is_empty() {
            return None;
        }
        let initializers = self
            .class
            .initializers
            .iter()
            .map(|initializer| {
                let definition = self
                    .definition
                    .member(initializer.id.into())
                    .expect("resolved initializer declaration must have a matching body");
                self.check_member(ClassMemberContext {
                    callable: initializer.id.into(),
                    parameters: &initializer.parameters,
                    definition,
                    return_type: Type::Unit,
                    access: HirAccess::Mutable,
                    body_kind: MemberBodyKind::OrdinaryInitializer,
                    callable_name: format!("initializer for class `{}`", self.class.name),
                })
            })
            .collect();
        let copy_constructor = self
            .class
            .copy_constructor_declaration
            .as_ref()
            .map(|copy| {
                self.check_member(ClassMemberContext {
                    callable: copy.id.into(),
                    parameters: &copy.parameters,
                    definition: self
                        .definition
                        .copy_constructor
                        .as_ref()
                        .expect("resolved copy-constructor declaration must have a body"),
                    return_type: Type::Unit,
                    access: HirAccess::Mutable,
                    body_kind: MemberBodyKind::CopyConstructor,
                    callable_name: format!("copy constructor for class `{}`", self.class.name),
                })
            });
        let copy_assignment = self.class.copy_assignment_declaration.as_ref().map(|copy| {
            self.check_member(ClassMemberContext {
                callable: copy.id.into(),
                parameters: std::slice::from_ref(&copy.parameter),
                definition: self
                    .definition
                    .copy_assignment
                    .as_ref()
                    .expect("resolved copy-assignment declaration must have a body"),
                return_type: Type::Unit,
                access: HirAccess::Mutable,
                body_kind: MemberBodyKind::CopyAssignment,
                callable_name: format!("copy assignment for class `{}`", self.class.name),
            })
        });
        let destructor = self.class.destructor.as_ref().map(|destructor| {
            self.check_member(ClassMemberContext {
                callable: destructor.id.into(),
                parameters: &[],
                definition: self
                    .definition
                    .destructor
                    .as_ref()
                    .expect("resolved destructor declaration must have a body"),
                return_type: Type::Unit,
                access: DESTRUCTOR_RECEIVER_ACCESS,
                body_kind: MemberBodyKind::MethodOrDestructor,
                callable_name: format!("destructor for class `{}`", self.class.name),
            })
        });
        let methods = self
            .class
            .methods
            .iter()
            .zip(&self.definition.methods)
            .map(|(method, body)| {
                self.check_member(ClassMemberContext {
                    callable: method.id.into(),
                    parameters: &method.parameters,
                    definition: body,
                    return_type: lower_type(&method.return_type),
                    access: lower_receiver_access(method.receiver_access),
                    body_kind: MemberBodyKind::MethodOrDestructor,
                    callable_name: format!("method `{}`", method.name),
                })
            })
            .collect();
        Some(HirClassDefinition {
            class: self.class.id,
            initializers,
            copy_constructor,
            copy_assignment,
            destructor,
            methods,
            span: self.definition.span,
        })
    }

    fn check_member(&mut self, context: ClassMemberContext<'_>) -> HirMemberDefinition {
        CallableChecker::new_member(
            self.program,
            self.copy_capabilities,
            MemberCheckContext {
                callable: context.callable,
                parameters: context.parameters,
                definition: context.definition,
                return_type: context.return_type,
                receiver: ReceiverContext {
                    class: self.class.id,
                    access: context.access,
                    body_kind: context.body_kind,
                },
                callable_name: context.callable_name,
            },
            self.diagnostics,
        )
        .check_member()
    }
}

struct ClassMemberContext<'program> {
    callable: CallableId,
    parameters: &'program [ResolvedParameter],
    definition: &'program ResolvedMemberDefinition,
    return_type: Type,
    access: HirAccess,
    body_kind: MemberBodyKind,
    callable_name: String,
}

const fn lower_receiver_access(access: ResolvedReceiverAccess) -> HirAccess {
    match access {
        ResolvedReceiverAccess::ReadOnly => HirAccess::ReadOnly,
        ResolvedReceiverAccess::Mutable => HirAccess::Mutable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::ClassId,
        test_support::{resolve_source, type_check_source},
    };

    #[test]
    fn preserves_class_table_order_and_optional_definition_slots() {
        let output = type_check_source(concat!(
            "class Complete {\n",
            "    value: i64;\n",
            "    init(value: i64) { self.value = value; }\n",
            "    copy(ref source: Complete) { self.value = source.value; }\n",
            "    assign(ref source: Complete) { self.value = source.value; }\n",
            "    destroy {}\n",
            "    fn read() -> i64 { return self.value; }\n",
            "}\n",
            "class Minimal { init() {} }\n",
            "fn main() -> i64 { return 0; }\n",
        ));

        assert!(!output.has_errors(), "{:?}", output.diagnostics);
        let hir = output.hir.unwrap();
        let classes: Vec<_> = hir.classes.iter().map(|class| class.id).collect();
        assert_eq!(classes, [ClassId::new(0), ClassId::new(1)]);

        let complete = hir.class_definitions.get(ClassId::new(0)).unwrap();
        assert!(complete.copy_constructor.is_some());
        assert!(complete.copy_assignment.is_some());
        assert!(complete.destructor.is_some());
        assert_eq!(complete.methods.len(), 1);

        let minimal = hir.class_definitions.get(ClassId::new(1)).unwrap();
        assert!(minimal.copy_constructor.is_none());
        assert!(minimal.copy_assignment.is_none());
        assert!(minimal.destructor.is_none());
        assert!(minimal.methods.is_empty());
    }

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
            Vec::new(),
            &mut diagnostics,
        )
        .expect("class-typed fields should lower to HIR declarations");

        assert!(diagnostics.is_empty());
        assert_eq!(outer.fields[0].ty, Type::Class(ClassId::new(1)));
    }

    #[test]
    fn validated_inheritance_crosses_type_check_with_explicit_base_lifecycle() {
        let output = type_check_source(concat!(
            "class Derived extends Base { init() { super(); } }\n",
            "class Base { init() {} }\n",
            "fn main() -> i64 { return 0; }\n",
        ));

        assert!(output.diagnostics.is_empty());
        let hir = output.hir.unwrap();
        let derived = hir.class(ClassId::new(0)).unwrap();
        assert_eq!(
            derived.direct_base.as_ref().map(|base| base.class),
            Some(ClassId::new(1))
        );
        assert_eq!(
            derived.destruction.steps.last(),
            Some(&crate::hir::HirDestructionStep::Base(ClassId::new(1)))
        );
    }
}
