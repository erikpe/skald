//! Closed compiler-known intrinsic registry and canonical declaration checks.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::ModuleId,
    intrinsic::Intrinsic,
    module::{ModulePath, ProgramModuleTable},
    resolve::{
        ResolvedFunctionDeclaration, ResolvedFunctionDeclarationTable, ResolvedFunctionLinkage,
        ResolvedModuleDeclarationTable, ResolvedParameterBindingMode, ResolvedTopLevelId,
        ResolvedTypeKind, ResolvedVisibility,
    },
    source::Span,
};

use super::super::{ResolvedTypeInterner, INVALID_INTRINSIC_DECLARATION};

const ERROR_MODULE_PATH: &str = "std::error";
const F64_MODULE_PATH: &str = "std::f64";
const IO_MODULE_PATH: &str = "std::io";
const STRING_MODULE_PATH: &str = "std::str";

#[derive(Clone, Copy)]
struct RegistryEntry {
    module_path: &'static str,
    name: &'static str,
    intrinsic: Intrinsic,
}

const REGISTRY: &[RegistryEntry] = &[
    RegistryEntry {
        module_path: ERROR_MODULE_PATH,
        name: "panic",
        intrinsic: Intrinsic::Panic,
    },
    RegistryEntry {
        module_path: IO_MODULE_PATH,
        name: "_io_standard_handle",
        intrinsic: Intrinsic::IoStandardHandle,
    },
    RegistryEntry {
        module_path: IO_MODULE_PATH,
        name: "_io_open",
        intrinsic: Intrinsic::IoOpen,
    },
    RegistryEntry {
        module_path: IO_MODULE_PATH,
        name: "_io_read",
        intrinsic: Intrinsic::IoRead,
    },
    RegistryEntry {
        module_path: IO_MODULE_PATH,
        name: "_io_write",
        intrinsic: Intrinsic::IoWrite,
    },
    RegistryEntry {
        module_path: IO_MODULE_PATH,
        name: "_io_close",
        intrinsic: Intrinsic::IoClose,
    },
    RegistryEntry {
        module_path: F64_MODULE_PATH,
        name: "_to_bits",
        intrinsic: Intrinsic::F64ToBits,
    },
    RegistryEntry {
        module_path: F64_MODULE_PATH,
        name: "_from_bits",
        intrinsic: Intrinsic::F64FromBits,
    },
];

pub(super) fn intrinsic_for_declaration(
    modules: &ProgramModuleTable,
    module: ModuleId,
    name: &str,
) -> Option<Intrinsic> {
    let path = modules.get(module)?.module_path();
    REGISTRY
        .iter()
        .find(|entry| {
            entry.name == name
                && path
                    == &ModulePath::try_from(entry.module_path)
                        .expect("canonical intrinsic module path is valid")
        })
        .map(|entry| entry.intrinsic)
}

pub(super) fn validate_intrinsic_declarations(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    functions: &ResolvedFunctionDeclarationTable,
    type_interner: &ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) {
    for declaration in functions
        .iter()
        .filter(|declaration| declaration.linkage == ResolvedFunctionLinkage::UnrecognizedIntrinsic)
    {
        let module_path = modules
            .get(declaration.module)
            .expect("resolved declaration module must exist")
            .module_path();
        let io_candidate = module_path
            == &ModulePath::try_from(IO_MODULE_PATH).expect("canonical I/O path is valid")
            || declaration.name.starts_with("_io_");
        let f64_candidate = module_path
            == &ModulePath::try_from(F64_MODULE_PATH).expect("canonical f64 path is valid");
        let diagnostic = Diagnostic::error(
            INVALID_INTRINSIC_DECLARATION,
            "intrinsic functions are reserved for compiler-defined declarations",
        );
        diagnostics.push(if io_candidate || f64_candidate {
            diagnostic
                .with_primary_label(
                    declaration.span,
                    "this is not a canonical standard-library intrinsic declaration",
                )
                .with_note(
                    "only the canonical declarations in `std::error`, `std::f64`, and `std::io` \
                     are recognized",
                )
        } else {
            // Preserve the original panic-registry diagnostic for source
            // outside the newly reserved std::io namespace.
            diagnostic
                .with_primary_label(
                    declaration.span,
                    "this is not the canonical `std::error::panic` declaration",
                )
                .with_note(
                    "only `public intrinsic fn panic(message: std::str::Str) -> unit;` \
                     in `std::error` is recognized",
                )
        });
    }

    validate_panic_intrinsic(modules, module_declarations, functions, diagnostics);
    validate_f64_intrinsics(modules, module_declarations, functions, diagnostics);
    validate_io_intrinsics(
        modules,
        module_declarations,
        functions,
        type_interner,
        diagnostics,
    );
}

fn validate_f64_intrinsics(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    functions: &ResolvedFunctionDeclarationTable,
    diagnostics: &mut Diagnostics,
) {
    let Some(f64_module) = canonical_module(modules, F64_MODULE_PATH) else {
        return;
    };
    let declarations = module_declarations
        .get(f64_module)
        .expect("every loaded module has a declaration table");

    for specification in F64_INTRINSICS {
        let qualified_name = format!("`std::f64::{}`", specification.name);
        let Some(indexed) = declarations.get(specification.name) else {
            let source_id = modules
                .get(f64_module)
                .expect("canonical f64 module must be loaded")
                .source_id();
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!(
                        "`std::f64` must declare the canonical `{}` intrinsic",
                        specification.name
                    ),
                )
                .with_primary_label(
                    Span::empty(source_id, 0),
                    format!("add `{}`", specification.source_signature),
                ),
            );
            continue;
        };
        let ResolvedTopLevelId::Function(function_id) = indexed.declaration else {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!("{qualified_name} must be an intrinsic function"),
                )
                .with_primary_label(indexed.name_span, "declared with the wrong kind"),
            );
            continue;
        };
        let declaration = functions
            .get(function_id)
            .expect("resolved function declaration identity must exist");
        if !matches!(
            declaration.linkage,
            ResolvedFunctionLinkage::Intrinsic { intrinsic }
                if intrinsic == specification.intrinsic
        ) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!("{qualified_name} must use `intrinsic fn`"),
                )
                .with_primary_label(
                    declaration.span,
                    "ordinary and external functions are not f64 bit intrinsics",
                ),
            );
            continue;
        }
        if declaration.visibility != ResolvedVisibility::Private {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!("{qualified_name} must be private"),
                )
                .with_primary_label(declaration.name_span, "public intrinsic declaration"),
            );
        }
        if declaration.parameters.len() != 1 {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!("{qualified_name} must declare one parameter"),
                )
                .with_primary_label(
                    declaration.name_span,
                    format!("found {} parameters", declaration.parameters.len()),
                ),
            );
        } else {
            let parameter = &declaration.parameters[0];
            if parameter.name != specification.parameter_name {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTRINSIC_DECLARATION,
                        format!(
                            "the {qualified_name} parameter must be named `{}`",
                            specification.parameter_name
                        ),
                    )
                    .with_primary_label(
                        parameter.name_span,
                        format!(
                            "rename this parameter to `{}`",
                            specification.parameter_name
                        ),
                    ),
                );
            }
            if parameter.binding_mode != ResolvedParameterBindingMode::Value {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTRINSIC_DECLARATION,
                        format!("the {qualified_name} parameter must be passed by value"),
                    )
                    .with_primary_label(parameter.span, "wrong parameter binding mode"),
                );
            }
            if parameter.type_syntax.kind != specification.parameter_type {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTRINSIC_DECLARATION,
                        format!(
                            "the {qualified_name} parameter must have exact type `{}`",
                            specification.parameter_type_name
                        ),
                    )
                    .with_primary_label(
                        parameter.type_syntax.span,
                        "wrong intrinsic parameter type",
                    ),
                );
            }
        }
        if declaration.return_type.kind != specification.return_type {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!(
                        "{qualified_name} must return `{}`",
                        specification.return_type_name
                    ),
                )
                .with_primary_label(declaration.return_type.span, "wrong intrinsic result type"),
            );
        }
    }
}

#[derive(Clone, Copy)]
struct F64IntrinsicSpecification {
    intrinsic: Intrinsic,
    name: &'static str,
    parameter_name: &'static str,
    parameter_type: ResolvedTypeKind,
    parameter_type_name: &'static str,
    return_type: ResolvedTypeKind,
    return_type_name: &'static str,
    source_signature: &'static str,
}

const F64_INTRINSICS: &[F64IntrinsicSpecification] = &[
    F64IntrinsicSpecification {
        intrinsic: Intrinsic::F64ToBits,
        name: "_to_bits",
        parameter_name: "value",
        parameter_type: ResolvedTypeKind::F64,
        parameter_type_name: "f64",
        return_type: ResolvedTypeKind::U64,
        return_type_name: "u64",
        source_signature: "intrinsic fn _to_bits(value: f64) -> u64;",
    },
    F64IntrinsicSpecification {
        intrinsic: Intrinsic::F64FromBits,
        name: "_from_bits",
        parameter_name: "bits",
        parameter_type: ResolvedTypeKind::U64,
        parameter_type_name: "u64",
        return_type: ResolvedTypeKind::F64,
        return_type_name: "f64",
        source_signature: "intrinsic fn _from_bits(bits: u64) -> f64;",
    },
];

fn validate_io_intrinsics(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    functions: &ResolvedFunctionDeclarationTable,
    type_interner: &ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) {
    let Some(io_module) = canonical_module(modules, IO_MODULE_PATH) else {
        return;
    };
    let declarations = module_declarations
        .get(io_module)
        .expect("every loaded module has a declaration table");

    for specification in IO_INTRINSICS {
        let Some(indexed) = declarations.get(specification.name) else {
            let source_id = modules
                .get(io_module)
                .expect("canonical I/O module must be loaded")
                .source_id();
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!(
                        "`std::io` must declare the canonical `{}` intrinsic",
                        specification.name
                    ),
                )
                .with_primary_label(
                    Span::empty(source_id, 0),
                    format!("add `{}`", specification.source_signature),
                ),
            );
            continue;
        };
        let ResolvedTopLevelId::Function(function_id) = indexed.declaration else {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!(
                        "`std::io::{}` must be an intrinsic function",
                        specification.name
                    ),
                )
                .with_primary_label(indexed.name_span, "declared with the wrong kind"),
            );
            continue;
        };
        let declaration = functions
            .get(function_id)
            .expect("resolved function declaration identity must exist");
        if !matches!(
            declaration.linkage,
            ResolvedFunctionLinkage::Intrinsic { intrinsic }
                if intrinsic == specification.intrinsic
        ) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    format!("`std::io::{}` must use `intrinsic fn`", specification.name),
                )
                .with_primary_label(
                    declaration.span,
                    "ordinary and external functions are not I/O intrinsics",
                ),
            );
            continue;
        }
        validate_io_signature(declaration, specification, type_interner, diagnostics);
    }
}

fn validate_io_signature(
    declaration: &ResolvedFunctionDeclaration,
    specification: &IoIntrinsicSpecification,
    type_interner: &ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) {
    let qualified_name = format!("`std::io::{}`", specification.name);
    if declaration.visibility != ResolvedVisibility::Private {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                format!("{qualified_name} must be private"),
            )
            .with_primary_label(declaration.name_span, "public intrinsic declaration"),
        );
    }

    if declaration.parameters.len() != specification.parameters.len() {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                format!(
                    "{qualified_name} must declare {} parameter{}",
                    specification.parameters.len(),
                    if specification.parameters.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
            )
            .with_primary_label(
                declaration.name_span,
                format!("found {} parameters", declaration.parameters.len()),
            ),
        );
    } else {
        for (parameter, expected) in declaration.parameters.iter().zip(specification.parameters) {
            if parameter.name != expected.name {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTRINSIC_DECLARATION,
                        format!(
                            "the {qualified_name} parameter must be named `{}`",
                            expected.name
                        ),
                    )
                    .with_primary_label(
                        parameter.name_span,
                        format!("rename this parameter to `{}`", expected.name),
                    ),
                );
            }
            if !expected.mode.matches(parameter.binding_mode) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTRINSIC_DECLARATION,
                        format!(
                            "the {qualified_name} `{}` parameter must be passed {}",
                            expected.name,
                            expected.mode.description()
                        ),
                    )
                    .with_primary_label(parameter.span, "wrong parameter binding mode"),
                );
            }
            if !expected
                .ty
                .matches(parameter.type_syntax.kind, type_interner)
            {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTRINSIC_DECLARATION,
                        format!(
                            "the {qualified_name} `{}` parameter must have exact type `{}`",
                            expected.name,
                            expected.ty.name()
                        ),
                    )
                    .with_primary_label(
                        parameter.type_syntax.span,
                        "wrong intrinsic parameter type",
                    ),
                );
            }
        }
    }

    if declaration.return_type.kind != ResolvedTypeKind::I64 {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                format!("{qualified_name} must return `i64`"),
            )
            .with_primary_label(declaration.return_type.span, "wrong intrinsic result type"),
        );
    }
}

#[derive(Clone, Copy)]
struct IoIntrinsicSpecification {
    intrinsic: Intrinsic,
    name: &'static str,
    parameters: &'static [ParameterSpecification],
    source_signature: &'static str,
}

#[derive(Clone, Copy)]
struct ParameterSpecification {
    name: &'static str,
    mode: ParameterMode,
    ty: ParameterType,
}

#[derive(Clone, Copy)]
enum ParameterMode {
    Value,
    ReadOnlyAlias,
    MutableAlias,
}

impl ParameterMode {
    const fn matches(self, actual: ResolvedParameterBindingMode) -> bool {
        matches!(
            (self, actual),
            (Self::Value, ResolvedParameterBindingMode::Value)
                | (
                    Self::ReadOnlyAlias,
                    ResolvedParameterBindingMode::ReadOnlyAlias { .. }
                )
                | (
                    Self::MutableAlias,
                    ResolvedParameterBindingMode::MutableAlias { .. }
                )
        )
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Value => "by value",
            Self::ReadOnlyAlias => "by read-only alias",
            Self::MutableAlias => "by mutable alias",
        }
    }
}

#[derive(Clone, Copy)]
enum ParameterType {
    I64,
    U64,
    U8,
    U8Array,
}

impl ParameterType {
    fn matches(self, actual: ResolvedTypeKind, type_interner: &ResolvedTypeInterner) -> bool {
        match (self, actual) {
            (Self::I64, ResolvedTypeKind::I64)
            | (Self::U64, ResolvedTypeKind::U64)
            | (Self::U8, ResolvedTypeKind::U8) => true,
            (Self::U8Array, ResolvedTypeKind::Array(array)) => type_interner
                .array(array)
                .is_some_and(|array| array.element.kind == ResolvedTypeKind::U8),
            _ => false,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::U8 => "u8",
            Self::U8Array => "u8[]",
        }
    }
}

const VALUE_U8: &[ParameterSpecification] = &[ParameterSpecification {
    name: "stream",
    mode: ParameterMode::Value,
    ty: ParameterType::U8,
}];
const OPEN_PARAMETERS: &[ParameterSpecification] = &[
    ParameterSpecification {
        name: "path",
        mode: ParameterMode::ReadOnlyAlias,
        ty: ParameterType::U8Array,
    },
    ParameterSpecification {
        name: "mode",
        mode: ParameterMode::Value,
        ty: ParameterType::U8,
    },
];
const READ_PARAMETERS: &[ParameterSpecification] = &[
    ParameterSpecification {
        name: "handle",
        mode: ParameterMode::Value,
        ty: ParameterType::I64,
    },
    ParameterSpecification {
        name: "destination",
        mode: ParameterMode::MutableAlias,
        ty: ParameterType::U8Array,
    },
    ParameterSpecification {
        name: "offset",
        mode: ParameterMode::Value,
        ty: ParameterType::U64,
    },
];
const WRITE_PARAMETERS: &[ParameterSpecification] = &[
    ParameterSpecification {
        name: "handle",
        mode: ParameterMode::Value,
        ty: ParameterType::I64,
    },
    ParameterSpecification {
        name: "source",
        mode: ParameterMode::ReadOnlyAlias,
        ty: ParameterType::U8Array,
    },
    ParameterSpecification {
        name: "offset",
        mode: ParameterMode::Value,
        ty: ParameterType::U64,
    },
];
const HANDLE_PARAMETER: &[ParameterSpecification] = &[ParameterSpecification {
    name: "handle",
    mode: ParameterMode::Value,
    ty: ParameterType::I64,
}];

const IO_INTRINSICS: &[IoIntrinsicSpecification] = &[
    IoIntrinsicSpecification {
        intrinsic: Intrinsic::IoStandardHandle,
        name: "_io_standard_handle",
        parameters: VALUE_U8,
        source_signature: "intrinsic fn _io_standard_handle(stream: u8) -> i64;",
    },
    IoIntrinsicSpecification {
        intrinsic: Intrinsic::IoOpen,
        name: "_io_open",
        parameters: OPEN_PARAMETERS,
        source_signature: "intrinsic fn _io_open(ref path: u8[], mode: u8) -> i64;",
    },
    IoIntrinsicSpecification {
        intrinsic: Intrinsic::IoRead,
        name: "_io_read",
        parameters: READ_PARAMETERS,
        source_signature:
            "intrinsic fn _io_read(handle: i64, mut ref destination: u8[], offset: u64) -> i64;",
    },
    IoIntrinsicSpecification {
        intrinsic: Intrinsic::IoWrite,
        name: "_io_write",
        parameters: WRITE_PARAMETERS,
        source_signature:
            "intrinsic fn _io_write(handle: i64, ref source: u8[], offset: u64) -> i64;",
    },
    IoIntrinsicSpecification {
        intrinsic: Intrinsic::IoClose,
        name: "_io_close",
        parameters: HANDLE_PARAMETER,
        source_signature: "intrinsic fn _io_close(handle: i64) -> i64;",
    },
];

fn canonical_module(modules: &ProgramModuleTable, path: &str) -> Option<ModuleId> {
    modules
        .find(&ModulePath::try_from(path).expect("canonical intrinsic module path is valid"))
        .map(|module| module.module_id())
}

// Keep the established panic validation and its diagnostics exact.
fn validate_panic_intrinsic(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    functions: &ResolvedFunctionDeclarationTable,
    diagnostics: &mut Diagnostics,
) {
    let canonical_error_module = canonical_module(modules, ERROR_MODULE_PATH);

    let Some(error_module) = canonical_error_module else {
        return;
    };
    let declarations = module_declarations
        .get(error_module)
        .expect("every loaded module has a declaration table");
    let Some(indexed) = declarations.get("panic") else {
        let source_id = modules
            .get(error_module)
            .expect("canonical error module must be loaded")
            .source_id();
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error` must declare the canonical `panic` intrinsic",
            )
            .with_primary_label(
                Span::empty(source_id, 0),
                "add `public intrinsic fn panic(message: std::str::Str) -> unit;`",
            ),
        );
        return;
    };
    let ResolvedTopLevelId::Function(function_id) = indexed.declaration else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must be an intrinsic function",
            )
            .with_primary_label(indexed.name_span, "declared with the wrong kind"),
        );
        return;
    };
    let declaration = functions
        .get(function_id)
        .expect("resolved function declaration identity must exist");
    if !matches!(
        declaration.linkage,
        ResolvedFunctionLinkage::Intrinsic {
            intrinsic: Intrinsic::Panic
        }
    ) {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must use `intrinsic fn`",
            )
            .with_primary_label(
                declaration.span,
                "ordinary and external functions are not panic",
            ),
        );
        return;
    }

    if declaration.visibility != ResolvedVisibility::Public {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must be public",
            )
            .with_primary_label(declaration.name_span, "private intrinsic declaration"),
        );
    }

    let string_class = canonical_module(modules, STRING_MODULE_PATH)
        .and_then(|module| module_declarations.get(module))
        .and_then(|declarations| declarations.get("Str"))
        .and_then(|declaration| match declaration.declaration {
            ResolvedTopLevelId::Class(class) => Some(class),
            _ => None,
        });

    if declaration.parameters.len() != 1 {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must declare one parameter",
            )
            .with_primary_label(
                declaration.name_span,
                format!("found {} parameters", declaration.parameters.len()),
            ),
        );
    } else {
        let parameter = &declaration.parameters[0];
        if parameter.name != "message" {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    "the `std::error::panic` parameter must be named `message`",
                )
                .with_primary_label(parameter.name_span, "rename this parameter to `message`"),
            );
        }
        if parameter.binding_mode != ResolvedParameterBindingMode::Value {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    "the `std::error::panic` parameter must be passed by value",
                )
                .with_primary_label(parameter.span, "alias parameters are not allowed"),
            );
        }
        if string_class.is_none()
            || !matches!(
                parameter.type_syntax.kind,
                ResolvedTypeKind::Class(class) if Some(class) == string_class
            )
        {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    "the `std::error::panic` parameter must have exact type `std::str::Str`",
                )
                .with_primary_label(parameter.type_syntax.span, "wrong panic message type"),
            );
        }
    }

    if declaration.return_type.kind != ResolvedTypeKind::Unit {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must return `unit`",
            )
            .with_primary_label(declaration.return_type.span, "wrong panic result type"),
        );
    }
}
