//! Deterministic textual rendering of typed HIR.

mod aggregate;
mod body;
mod call;
mod declarations;
mod expression;
mod ownership;

use std::fmt::Write;

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    source::Span,
};

use super::ir::*;

pub fn dump_hir(program: &HirProgram) -> String {
    let mut dumper = HirDumper::new(
        &program.function_types,
        &program.optional_types,
        &program.optional_box_types,
    );
    dumper.line("HirProgram", program.span);
    dumper.indented(|dumper| {
        dumper.raw_line(&format!("SelectedModule {}", program.modules.selected()));
        dumper.heading("Modules");
        dumper.indented(|dumper| {
            for module in program.modules.iter() {
                dumper.raw_line(&format!(
                    "Module {} {} source {} provider {} package {}",
                    module.module_id(),
                    module.module_path(),
                    module.source_id().index(),
                    module.provider_id(),
                    module.package_id()
                ));
            }
        });
        if let Some(item) = &program.string_language_item {
            dumper.raw_line(&format!(
                "StringLanguageItem class {} fields {} {} {} {}",
                item.class,
                item.storage_field,
                item.start_field,
                item.length_field,
                item.hash_code_field
            ));
        }
        if !program.optional_types.is_empty() {
            dumper.heading("OptionalTypes");
            dumper.indented(|dumper| {
                for optional in program.optional_types.iter() {
                    dumper.raw_line(&format!(
                        "OptionalType {} payload {} storage={:?} representation={:?}",
                        optional.id,
                        dumper.type_name(optional.payload),
                        optional.storage,
                        optional.representation,
                    ));
                    dumper.indented(|dumper| {
                        dumper.raw_line(&format!(
                            "Lifecycle initialization={:?} injection={:?} copy={:?} assignment={:?} destruction={:?} presence={:?} unwrap={:?} checked={:?}",
                            optional.lifecycle.initialization,
                            optional.lifecycle.injection,
                            optional.lifecycle.copy,
                            optional.lifecycle.assignment,
                            optional.lifecycle.destruction,
                            optional.lifecycle.presence_test,
                            optional.lifecycle.unwrap,
                            optional.checked_access,
                        ));
                        dumper.raw_line(&format!(
                            "Boundaries argument={:?} result={:?} static={:?} array-element={:?}",
                            optional.boundaries.argument,
                            optional.boundaries.result,
                            optional.boundaries.static_storage,
                            optional.boundaries.array_element,
                        ));
                    });
                }
            });
        }
        if !program.optional_box_types.is_empty() {
            dumper.heading("OptionalBoxTypes");
            dumper.indented(|dumper| {
                for target in program.optional_box_types.iter() {
                    dumper.raw_line(&format!(
                        "OptionalBoxType {} exact={} dynamic={:?} depth={} view={:?}",
                        target.id,
                        target
                            .exact_optional
                            .map(|optional| optional.to_string())
                            .unwrap_or_else(|| "view-only".to_owned()),
                        target.exact_dynamic_class,
                        target.optional_depth,
                        target.object_view,
                    ));
                }
            });
        }
        if !program.literal_data.is_empty() {
            dumper.heading("LiteralData");
            dumper.indented(|dumper| {
                for literal in program.literal_data.iter() {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "{} bytes=", literal.id);
                    for byte in &literal.bytes {
                        let _ = write!(dumper.output, "{byte:02x}");
                    }
                    write_span(&mut dumper.output, literal.span);
                    dumper.output.push('\n');
                }
            });
        }
        if !program.external_links.is_empty() {
            dumper.heading("ExternalLinks");
            dumper.indented(|dumper| {
                for link in program.external_links.iter() {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Link {} ", link.id);
                    write_quoted(&mut dumper.output, &link.symbol);
                    dumper.output.push_str(" declarations");
                    for declaration in &link.declarations {
                        let _ = write!(dumper.output, " {declaration}");
                    }
                    dumper.output.push('\n');
                }
            });
        }
        dumper.write_indentation();
        let _ = writeln!(dumper.output, "Entry {}", program.entry_function);
        if !program.function_types.is_empty() {
            dumper.heading("FunctionTypes");
            dumper.indented(|dumper| {
                for function in program.function_types.iter() {
                    dumper.line(
                        &format!(
                            "FunctionType {} -> {}",
                            function.id,
                            dumper.type_name(function.result)
                        ),
                        function.span,
                    );
                    dumper.indented(|dumper| {
                        for parameter in &function.parameters {
                            dumper.line(
                                &format!(
                                    "Parameter {:?} {}",
                                    parameter.mode,
                                    dumper.type_name(parameter.ty)
                                ),
                                parameter.span,
                            );
                        }
                    });
                }
            });
        }
        if !program.array_types.is_empty() {
            dumper.heading("ArrayTypes");
            dumper.indented(|dumper| {
                for array in program.array_types.iter() {
                    dumper.array_type(array);
                }
            });
        }
        if !program.classes.is_empty() {
            dumper.heading("Classes");
            dumper.indented(|dumper| {
                for class in program.classes.iter() {
                    dumper.class_declaration(class);
                }
            });
            dumper.heading("ClassDefinitions");
            dumper.indented(|dumper| {
                for class in program.class_definitions.iter() {
                    dumper.class_definition(class);
                }
            });
        }
        if !program.interfaces.is_empty() {
            dumper.heading("Interfaces");
            dumper.indented(|dumper| {
                for interface in program.interfaces.iter() {
                    dumper.interface_declaration(interface);
                }
            });
        }
        if !program.virtual_families.is_empty() {
            dumper.heading("VirtualFamilies");
            dumper.indented(|dumper| {
                for family in program.virtual_families.iter() {
                    dumper.raw_line(&format!(
                        "Family {} slot {} root {}",
                        family.id, family.slot, family.root
                    ));
                }
            });
        }
        dumper.heading("Declarations");
        dumper.indented(|dumper| {
            for declaration in program.declarations.iter() {
                dumper.declaration(declaration);
            }
        });
        dumper.heading("Definitions");
        dumper.indented(|dumper| {
            for definition in program.definitions.iter() {
                dumper.definition(definition);
            }
        });
    });
    dumper.output
}

struct HirDumper<'types> {
    output: String,
    indentation: usize,
    function_types: &'types HirFunctionTypeTable,
    optional_types: &'types HirOptionalTypeTable,
    optional_box_types: &'types HirOptionalBoxTypeTable,
}

impl<'types> HirDumper<'types> {
    fn new(
        function_types: &'types HirFunctionTypeTable,
        optional_types: &'types HirOptionalTypeTable,
        optional_box_types: &'types HirOptionalBoxTypeTable,
    ) -> Self {
        Self {
            output: String::new(),
            indentation: 0,
            function_types,
            optional_types,
            optional_box_types,
        }
    }

    fn type_name(&self, ty: Type) -> String {
        match ty {
            Type::Function(function) => {
                let function = self
                    .function_types
                    .get(function)
                    .expect("HIR dump function identity must name metadata");
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| {
                        let mode = match parameter.mode {
                            HirFunctionTypeParameterMode::Value => "",
                            HirFunctionTypeParameterMode::ReadOnlyAlias => "ref ",
                            HirFunctionTypeParameterMode::MutableAlias => "mut ref ",
                        };
                        format!("{mode}{}", self.type_name(parameter.ty))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({parameters}) -> {}", self.type_name(function.result))
            }
            Type::Optional(optional) => {
                let payload = self
                    .optional_types
                    .get(optional)
                    .expect("HIR dump optional identity must name metadata")
                    .payload;
                let payload_name = self.type_name(payload);
                if matches!(payload, Type::Shared(_)) {
                    format!("({payload_name})?")
                } else {
                    format!("{payload_name}?")
                }
            }
            Type::Shared(HirSharedTarget::OptionalBox(target)) => {
                format!("shared {}", self.optional_box_target_name(target))
            }
            _ => ty.name().into_owned(),
        }
    }

    fn optional_box_target_name(&self, target: crate::identity::OptionalBoxTypeId) -> String {
        let target = self
            .optional_box_types
            .get(target)
            .expect("HIR dump optional-box identity must name metadata");
        if let Some(view) = target.object_view {
            let leaf = match view {
                HirViewTarget::Obj => "Obj".to_owned(),
                HirViewTarget::Class(class) => format!("class {class}"),
                HirViewTarget::Interface(interface) => format!("interface {interface}"),
            };
            return format!("{}{}", leaf, "?".repeat(target.optional_depth));
        }
        let optional = target
            .exact_optional
            .expect("non-object box target must retain an exact optional identity");
        self.type_name(Type::Optional(optional))
    }
}

impl<'types> HirDumper<'types> {
    fn typed_line(&mut self, name: &str, expression: &HirExpression) {
        self.write_indentation();
        let _ = write!(self.output, "{name} : {}", self.type_name(expression.ty));
        write_span(&mut self.output, expression.span);
        self.output.push('\n');
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn raw_line(&mut self, text: &str) {
        self.heading(text);
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
