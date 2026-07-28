//! Canonical `std::str::Str` selection and structural validation.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    module::{ModulePath, ProgramModuleTable},
    resolve::{
        ResolvedClassDeclarationTable, ResolvedCopyOperation, ResolvedLiteralData,
        ResolvedMemberVisibility, ResolvedModuleDeclarationTable, ResolvedSharedTarget,
        ResolvedStringLanguageItem, ResolvedTopLevelId, ResolvedTypeKind, ResolvedVisibility,
    },
};

use super::super::{ArrayTypeInterner, INVALID_STRING_LANGUAGE_ITEM};

pub(super) fn validate_string_language_item(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    classes: &ResolvedClassDeclarationTable,
    array_types: &ArrayTypeInterner,
    literal_data: &[ResolvedLiteralData],
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedStringLanguageItem> {
    let first_literal = literal_data.first()?.span;
    let path = ModulePath::try_from("std::str").expect("canonical string module path is valid");
    let module = modules
        .find(&path)
        .expect("string literal dependency must load the canonical module")
        .module_id();
    let declarations = module_declarations
        .get(module)
        .expect("every loaded module has a declaration table");
    let Some(declaration) = declarations.get("Str") else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str` does not declare the required `Str` class",
            )
            .with_primary_label(first_literal, "string language item required here"),
        );
        return None;
    };
    let ResolvedTopLevelId::Class(class_id) = declaration.declaration else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must be a class",
            )
            .with_primary_label(declaration.name_span, "declared with the wrong kind")
            .with_secondary_label(first_literal, "string language item required here"),
        );
        return None;
    };
    let class = classes
        .get(class_id)
        .expect("resolved class declaration identity must exist");
    let mut valid = true;

    if declaration.visibility != ResolvedVisibility::Public {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must be public",
            )
            .with_primary_label(class.name_span, "private language-item declaration")
            .with_secondary_label(first_literal, "string language item required here"),
        );
        valid = false;
    }
    if let Some(base) = class.direct_base {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must not have a base class",
            )
            .with_primary_label(base.span, "remove this direct base")
            .with_secondary_label(first_literal, "string language item required here"),
        );
        valid = false;
    }
    if class.fields.len() != 3 {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must declare exactly three direct fields",
            )
            .with_primary_label(
                class.name_span,
                format!("found {} direct fields", class.fields.len()),
            )
            .with_secondary_label(first_literal, "string language item required here"),
        );
        valid = false;
    }

    let expected = [
        ("storage", ExpectedFieldType::SharedU8Array),
        ("start", ExpectedFieldType::U64),
        ("length", ExpectedFieldType::U64),
    ];
    for (index, (name, expected_type)) in expected.into_iter().enumerate() {
        let Some(field) = class.fields.get(index) else {
            continue;
        };
        if field.name != name {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_STRING_LANGUAGE_ITEM,
                    format!(
                        "string descriptor field {} must be named `{name}`",
                        index + 1
                    ),
                )
                .with_primary_label(field.name_span, format!("found `{}`", field.name))
                .with_secondary_label(first_literal, "string language item required here"),
            );
            valid = false;
        }
        if !matches!(field.visibility, ResolvedMemberVisibility::Private { .. }) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_STRING_LANGUAGE_ITEM,
                    format!("string descriptor field `{name}` must be private"),
                )
                .with_primary_label(field.name_span, "public descriptor field")
                .with_secondary_label(first_literal, "string language item required here"),
            );
            valid = false;
        }
        if !expected_type.matches(field.type_syntax.kind, array_types) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_STRING_LANGUAGE_ITEM,
                    format!(
                        "string descriptor field `{name}` must have type `{}`",
                        expected_type.name()
                    ),
                )
                .with_primary_label(field.type_syntax.span, "field has the wrong type")
                .with_secondary_label(first_literal, "string language item required here"),
            );
            valid = false;
        }
    }

    if let Some(copy) = &class.copy_constructor_declaration {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must use synthesized copy construction",
            )
            .with_primary_label(copy.span, "explicit copy constructor is forbidden")
            .with_secondary_label(first_literal, "string language item required here"),
        );
        valid = false;
    }
    if let Some(assignment) = &class.copy_assignment_declaration {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must use synthesized copy assignment",
            )
            .with_primary_label(assignment.span, "explicit copy assignment is forbidden")
            .with_secondary_label(first_literal, "string language item required here"),
        );
        valid = false;
    }
    if let Some(destructor) = &class.destructor {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must use synthesized destruction",
            )
            .with_primary_label(destructor.span, "explicit destructor is forbidden")
            .with_secondary_label(first_literal, "string language item required here"),
        );
        valid = false;
    }
    if !matches!(
        (class.copy_constructor, class.copy_assignment),
        (
            ResolvedCopyOperation::Synthesized(copy),
            ResolvedCopyOperation::Synthesized(assignment)
        ) if copy == class.id && assignment == class.id
    ) {
        diagnostics.push(
            Diagnostic::error(
                INVALID_STRING_LANGUAGE_ITEM,
                "`std::str::Str` must support synthesized field-wise lifecycle",
            )
            .with_primary_label(class.name_span, "synthesized lifecycle is unavailable")
            .with_secondary_label(first_literal, "string language item required here"),
        );
        valid = false;
    }

    valid.then(|| ResolvedStringLanguageItem {
        class: class.id,
        storage_field: class.fields[0].id,
        start_field: class.fields[1].id,
        length_field: class.fields[2].id,
        declaration_span: class.span,
        requiring_literal_spans: literal_data.iter().map(|literal| literal.span).collect(),
    })
}

#[derive(Clone, Copy)]
enum ExpectedFieldType {
    SharedU8Array,
    U64,
}

impl ExpectedFieldType {
    fn matches(self, actual: ResolvedTypeKind, arrays: &ArrayTypeInterner) -> bool {
        match self {
            Self::U64 => actual == ResolvedTypeKind::U64,
            Self::SharedU8Array => {
                let ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(array)) = actual else {
                    return false;
                };
                arrays
                    .get(array)
                    .is_some_and(|array| array.element.kind == ResolvedTypeKind::U8)
            }
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::SharedU8Array => "shared u8[]",
            Self::U64 => "u64",
        }
    }
}
