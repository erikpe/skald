//! Stable, source-independent textual rendering of the source AST.

use std::fmt::Write;

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    source::Span,
};

use super::ast::*;

pub fn dump_ast(ast: &CompilationUnit) -> String {
    let mut dumper = AstDumper::default();
    dumper.line("CompilationUnit", ast.span);
    dumper.indented(|dumper| {
        for import in &ast.imports {
            dumper.import(import);
        }
        for declaration in &ast.declarations {
            match declaration {
                TopLevelDeclaration::Function(function) => dumper.function(function),
                TopLevelDeclaration::ExternalFunction(function) => {
                    dumper.external_function(function)
                }
                TopLevelDeclaration::IntrinsicFunction(function) => {
                    dumper.intrinsic_function(function)
                }
                TopLevelDeclaration::Class(class) => dumper.class(class),
                TopLevelDeclaration::Interface(interface) => dumper.interface(interface),
            }
        }
    });
    dumper.output
}

#[derive(Default)]
struct AstDumper {
    output: String,
    indentation: usize,
}

impl AstDumper {
    fn import(&mut self, import: &ImportDeclaration) {
        match import {
            ImportDeclaration::Module(import) => {
                self.line("ModuleImport", import.span);
                self.indented(|dumper| {
                    dumper.line("Import", import.import_span);
                    dumper.name_path("Module", &import.module);
                    if let (Some(as_span), Some(alias)) = (import.as_span, &import.alias) {
                        dumper.line("As", as_span);
                        dumper.named("Alias", &alias.text, alias.span);
                    }
                    dumper.line("Semicolon", import.semicolon_span);
                });
            }
            ImportDeclaration::Selective(import) => {
                self.line("SelectiveImport", import.span);
                self.indented(|dumper| {
                    dumper.line("From", import.from_span);
                    dumper.name_path("Module", &import.module);
                    dumper.line("Import", import.import_span);
                    for (index, item) in import.items.iter().enumerate() {
                        dumper.line("Item", item.span);
                        dumper.indented(|dumper| {
                            dumper.named("Name", &item.name.text, item.name.span);
                            if let (Some(as_span), Some(alias)) = (item.as_span, &item.alias) {
                                dumper.line("As", as_span);
                                dumper.named("Alias", &alias.text, alias.span);
                            }
                        });
                        if let Some(comma) = import.comma_spans.get(index) {
                            dumper.line("Comma", *comma);
                        }
                    }
                    dumper.line("Semicolon", import.semicolon_span);
                });
            }
        }
    }

    fn visibility(&mut self, visibility: Visibility) {
        if let Visibility::Public { span } = visibility {
            self.line("Public", span);
        }
    }

    fn member_visibility(&mut self, visibility: MemberVisibility) {
        if let MemberVisibility::Private { span } = visibility {
            self.line("Private", span);
        }
    }

    fn name_path(&mut self, label: &str, name: &Name) {
        self.named(label, &name.text, name.span);
        if name.is_qualified() {
            self.indented(|dumper| {
                for (index, component) in name.components().enumerate() {
                    dumper.named("Component", component.text, component.span);
                    if let Some(separator) = name.separator_spans().get(index) {
                        dumper.line("Separator", *separator);
                    }
                }
            });
        }
    }

    fn named_type(&mut self, label: &str, named: &NamedTypeSyntax) {
        if let Some(arguments) = &named.arguments {
            self.line(label, named.span);
            self.indented(|dumper| {
                dumper.name_path("Name", &named.name);
                dumper.line("Arguments", arguments.span);
                dumper.indented(|dumper| {
                    dumper.line("LeftAngle", arguments.left_angle_span);
                    for (index, argument) in arguments.arguments.iter().enumerate() {
                        dumper.type_syntax(argument);
                        if let Some(comma) = arguments.comma_spans.get(index) {
                            dumper.line("Comma", *comma);
                        }
                    }
                    dumper.line("RightAngle", arguments.right_angle_span);
                });
            });
        } else {
            self.name_path(label, &named.name);
        }
    }

    fn generic_parameters(&mut self, parameters: &GenericParameterList) {
        self.line("TypeParameters", parameters.span);
        self.indented(|dumper| {
            dumper.line("LeftAngle", parameters.left_angle_span);
            for (index, parameter) in parameters.parameters.iter().enumerate() {
                dumper.named("Parameter", &parameter.text, parameter.span);
                if let Some(comma) = parameters.comma_spans.get(index) {
                    dumper.line("Comma", *comma);
                }
            }
            dumper.line("RightAngle", parameters.right_angle_span);
        });
    }

    fn generic_where_clause(&mut self, clause: &GenericWhereClause) {
        self.line("WhereClause", clause.span);
        self.indented(|dumper| {
            dumper.line("Where", clause.where_span);
            for (index, requirement) in clause.requirements.iter().enumerate() {
                dumper.line("Requirement", requirement.span);
                dumper.indented(|dumper| {
                    dumper.named(
                        "Parameter",
                        &requirement.parameter.text,
                        requirement.parameter.span,
                    );
                    dumper.line("Colon", requirement.colon_span);
                    dumper.named_type("Interface", &requirement.interface);
                });
                if let Some(comma) = clause.comma_spans.get(index) {
                    dumper.line("Comma", *comma);
                }
            }
        });
    }

    fn interface(&mut self, interface: &InterfaceDecl) {
        self.line("Interface", interface.span);
        self.indented(|dumper| {
            dumper.visibility(interface.visibility);
            dumper.named("Name", &interface.name.text, interface.name.span);
            if let Some(parameters) = &interface.type_parameters {
                dumper.generic_parameters(parameters);
            }
            if let Some(clause) = &interface.where_clause {
                dumper.generic_where_clause(clause);
            }
            dumper.heading("Requirements");
            dumper.indented(|dumper| {
                for requirement in &interface.requirements {
                    dumper.line(
                        if requirement.mut_span.is_some() {
                            "Requirement Mutable"
                        } else {
                            "Requirement ReadOnly"
                        },
                        requirement.span,
                    );
                    dumper.indented(|dumper| {
                        dumper.named("Name", &requirement.name.text, requirement.name.span);
                        dumper.parameters(&requirement.parameters);
                        dumper.type_syntax(&requirement.return_type);
                    });
                }
            });
        });
    }

    fn class(&mut self, class: &ClassDecl) {
        self.line("Class", class.span);
        self.indented(|dumper| {
            dumper.visibility(class.visibility);
            dumper.named("Name", &class.name.text, class.name.span);
            if let Some(parameters) = &class.type_parameters {
                dumper.generic_parameters(parameters);
            }
            if let Some(base) = &class.direct_base {
                dumper.named_type("DirectBase", base);
            }
            for interface in &class.implemented_interfaces {
                dumper.named_type("Implements", interface);
            }
            if let Some(clause) = &class.where_clause {
                dumper.generic_where_clause(clause);
            }
            dumper.heading("Members");
            dumper.indented(|dumper| {
                for member in &class.members {
                    dumper.class_member(member);
                }
            });
        });
    }

    fn class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Field(field) => {
                self.line("Field", field.span);
                self.indented(|dumper| {
                    dumper.member_visibility(field.visibility);
                    if let Some(span) = field.cell_span {
                        dumper.line("Cell", span);
                    }
                    if let Some(span) = field.final_span {
                        dumper.line("Final", span);
                    }
                    dumper.named("Name", &field.name.text, field.name.span);
                    dumper.type_syntax(&field.type_syntax);
                });
            }
            ClassMember::StaticField(field) => {
                self.line("StaticField", field.span);
                self.indented(|dumper| {
                    dumper.member_visibility(field.visibility);
                    if let Some(span) = field.final_span {
                        dumper.line("Final", span);
                    }
                    dumper.line("Static", field.static_span);
                    dumper.named("Name", &field.name.text, field.name.span);
                    dumper.type_syntax(&field.type_syntax);
                    if let Some(initializer) = &field.initializer {
                        dumper.line("DeclarationInitializer", initializer.span);
                        dumper.indented(|dumper| {
                            dumper.line("Equal", initializer.equal_span);
                            dumper.expression(&initializer.expression);
                        });
                    }
                });
            }
            ClassMember::Initializer(initializer) => {
                self.line("Initializer", initializer.span);
                self.indented(|dumper| {
                    dumper.member_visibility(initializer.visibility);
                    dumper.line("Introducer", initializer.introducer_span);
                    dumper.parameters(&initializer.parameters);
                    dumper.block(&initializer.body);
                });
            }
            ClassMember::CopyConstructor(constructor) => {
                self.line("CopyConstructor", constructor.span);
                self.indented(|dumper| {
                    dumper.line("Introducer", constructor.introducer_span);
                    dumper.parameters(&constructor.parameters);
                    dumper.block(&constructor.body);
                });
            }
            ClassMember::CopyAssignment(assignment) => {
                self.line("CopyAssignment", assignment.span);
                self.indented(|dumper| {
                    dumper.line("Introducer", assignment.introducer_span);
                    dumper.parameters(&assignment.parameters);
                    dumper.block(&assignment.body);
                });
            }
            ClassMember::Destructor(destructor) => {
                self.line("Destructor", destructor.span);
                self.indented(|dumper| {
                    dumper.line("Introducer", destructor.introducer_span);
                    dumper.block(&destructor.body);
                });
            }
            ClassMember::Method(method) => {
                self.line(
                    if method.static_span.is_some() {
                        "Method Static"
                    } else if method.mut_span.is_some() {
                        "Method Mutable"
                    } else {
                        "Method ReadOnly"
                    },
                    method.span,
                );
                self.indented(|dumper| {
                    dumper.member_visibility(method.visibility);
                    if let Some(span) = method.static_span {
                        dumper.line("Static", span);
                    }
                    if let Some(modifier) = method.modifier {
                        match modifier {
                            MethodModifier::Virtual { span } => {
                                dumper.line("Modifier Virtual", span)
                            }
                            MethodModifier::Override { span } => {
                                dumper.line("Modifier Override", span)
                            }
                        }
                    }
                    if let Some(span) = method.mut_span {
                        dumper.line("Mut", span);
                    }
                    dumper.named("Name", &method.name.text, method.name.span);
                    dumper.parameters_and_return(&method.parameters, &method.return_type);
                    dumper.block(&method.body);
                });
            }
        }
    }

    fn function(&mut self, function: &FunctionDecl) {
        self.line("Function", function.span);
        self.indented(|dumper| {
            dumper.visibility(function.visibility);
            dumper.named("Name", &function.name.text, function.name.span);
            dumper.parameters_and_return(&function.parameters, &function.return_type);
            dumper.block(&function.body);
        });
    }

    fn external_function(&mut self, function: &ExternalFunctionDecl) {
        self.line("ExternalFunction", function.span);
        self.indented(|dumper| {
            dumper.visibility(function.visibility);
            dumper.named("Name", &function.name.text, function.name.span);
            dumper.parameters_and_return(&function.parameters, &function.return_type);
        });
    }

    fn intrinsic_function(&mut self, function: &IntrinsicFunctionDecl) {
        self.line("IntrinsicFunction", function.span);
        self.indented(|dumper| {
            dumper.visibility(function.visibility);
            dumper.line("Intrinsic", function.intrinsic_span);
            dumper.named("Name", &function.name.text, function.name.span);
            dumper.parameters_and_return(&function.parameters, &function.return_type);
        });
    }

    fn parameters_and_return(&mut self, parameters: &[Parameter], return_type: &TypeSyntax) {
        self.parameters(parameters);
        self.heading("ReturnType");
        self.indented(|dumper| dumper.type_syntax(return_type));
    }

    fn parameters(&mut self, parameters: &[Parameter]) {
        self.heading("Parameters");
        self.indented(|dumper| {
            for parameter in parameters {
                dumper.line("Parameter", parameter.span);
                dumper.indented(|dumper| {
                    dumper.parameter_binding_mode(parameter.binding_mode);
                    dumper.named("Name", &parameter.name.text, parameter.name.span);
                    dumper.type_syntax(&parameter.type_syntax);
                });
            }
        });
    }

    fn parameter_binding_mode(&mut self, mode: ParameterBindingMode) {
        match mode {
            ParameterBindingMode::Value => self.heading("Binding Value"),
            ParameterBindingMode::ReadOnlyAlias { ref_span } => {
                self.heading("Binding ReadOnlyAlias");
                self.indented(|dumper| dumper.line("Ref", ref_span));
            }
            ParameterBindingMode::MutableAlias { mut_span, ref_span } => {
                self.heading("Binding MutableAlias");
                self.indented(|dumper| {
                    dumper.line("Mut", mut_span);
                    dumper.line("Ref", ref_span);
                });
            }
        }
    }

    fn type_syntax(&mut self, type_syntax: &TypeSyntax) {
        let kind = match &type_syntax.kind {
            TypeKind::I64 => "I64",
            TypeKind::U64 => "U64",
            TypeKind::U8 => "U8",
            TypeKind::F64 => "F64",
            TypeKind::Bool => "Bool",
            TypeKind::Unit => "Unit",
            TypeKind::Function(function) => {
                self.line("Type Function", type_syntax.span);
                self.indented(|dumper| {
                    dumper.line("Fn", function.fn_span);
                    dumper.line("LeftParen", function.left_paren_span);
                    for (index, parameter) in function.parameters.iter().enumerate() {
                        dumper.line("Parameter", parameter.span);
                        dumper.indented(|dumper| {
                            match parameter.mode {
                                FunctionTypeParameterMode::Value => {
                                    dumper.heading("Mode Value");
                                }
                                FunctionTypeParameterMode::ReadOnlyAlias { ref_span } => {
                                    dumper.heading("Mode ReadOnlyAlias");
                                    dumper.indented(|dumper| dumper.line("Ref", ref_span));
                                }
                                FunctionTypeParameterMode::MutableAlias { mut_span, ref_span } => {
                                    dumper.heading("Mode MutableAlias");
                                    dumper.indented(|dumper| {
                                        dumper.line("Mut", mut_span);
                                        dumper.line("Ref", ref_span);
                                    });
                                }
                            }
                            dumper.type_syntax(&parameter.type_syntax);
                        });
                        if let Some(comma) = function.comma_spans.get(index) {
                            dumper.line("Comma", *comma);
                        }
                    }
                    dumper.line("RightParen", function.right_paren_span);
                    dumper.line("Arrow", function.arrow_span);
                    dumper.heading("Result");
                    dumper.indented(|dumper| dumper.type_syntax(&function.result));
                });
                return;
            }
            TypeKind::Shared { target, .. } => {
                self.line("Type Shared", type_syntax.span);
                self.indented(|dumper| {
                    if let TypeKind::Named(target) = &target.kind {
                        dumper.named_type("Target", target);
                    } else {
                        dumper.heading("Target");
                        dumper.indented(|dumper| dumper.type_syntax(target));
                    }
                });
                return;
            }
            TypeKind::Optional {
                payload,
                question_span,
                spelling,
            } => {
                let spelling = match spelling {
                    OptionalTypeSpelling::Postfix => "Postfix",
                    OptionalTypeSpelling::SharedShorthand => "SharedShorthand",
                };
                self.line(&format!("Type Optional {spelling}"), type_syntax.span);
                self.indented(|dumper| {
                    dumper.heading("Payload");
                    dumper.indented(|dumper| dumper.type_syntax(payload));
                    dumper.line("Question", *question_span);
                });
                return;
            }
            TypeKind::Grouped {
                left_paren_span,
                inner,
                right_paren_span,
            } => {
                self.line("Type Grouped", type_syntax.span);
                self.indented(|dumper| {
                    dumper.line("LeftParen", *left_paren_span);
                    dumper.type_syntax(inner);
                    dumper.line("RightParen", *right_paren_span);
                });
                return;
            }
            TypeKind::Array {
                element,
                left_bracket_span,
                right_bracket_span,
            } => {
                self.line("Type Array", type_syntax.span);
                self.indented(|dumper| {
                    dumper.heading("Element");
                    dumper.indented(|dumper| dumper.type_syntax(element));
                    dumper.line("LeftBracket", *left_bracket_span);
                    dumper.line("RightBracket", *right_bracket_span);
                });
                return;
            }
            TypeKind::Named(named) => {
                self.named_type("Type Named", named);
                return;
            }
        };
        self.line(&format!("Type {kind}"), type_syntax.span);
    }

    fn block(&mut self, block: &Block) {
        self.line("Block", block.span);
        self.indented(|dumper| {
            for statement in &block.statements {
                dumper.statement(statement);
            }
        });
    }

    fn statement(&mut self, statement: &Statement) {
        match statement {
            Statement::BaseInitialization(statement) => {
                self.line("BaseInitialization", statement.span);
                self.indented(|dumper| {
                    dumper.line("Super", statement.super_span);
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &statement.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            Statement::Local(local) => {
                self.line("Local", local.span);
                self.indented(|dumper| {
                    dumper.named("Name", &local.name.text, local.name.span);
                    dumper.type_syntax(&local.type_syntax);
                    dumper.heading("Initializer");
                    dumper.indented(|dumper| dumper.expression(&local.initializer));
                });
            }
            Statement::Return(statement) => {
                self.line("Return", statement.span);
                if let Some(value) = &statement.value {
                    self.indented(|dumper| dumper.expression(value));
                }
            }
            Statement::Break(statement) => {
                self.line("Break", statement.span);
                self.indented(|dumper| dumper.line("BreakKeyword", statement.break_span));
            }
            Statement::Continue(statement) => {
                self.line("Continue", statement.span);
                self.indented(|dumper| dumper.line("ContinueKeyword", statement.continue_span));
            }
            Statement::Expression(statement) => {
                self.line("ExpressionStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.expression));
            }
            Statement::Conditional(statement) => self.conditional(statement),
            Statement::While(statement) => {
                self.line("While", statement.span);
                self.indented(|dumper| {
                    dumper.line("WhileKeyword", statement.while_span);
                    dumper.heading("Condition");
                    dumper.indented(|dumper| dumper.expression(&statement.condition));
                    dumper.block(&statement.body);
                });
            }
            Statement::ForIn(statement) => {
                self.line("ForIn", statement.span);
                self.indented(|dumper| {
                    dumper.line("ForKeyword", statement.for_span);
                    dumper.line("LeftParen", statement.left_paren_span);
                    dumper.named("Binding", &statement.binding.text, statement.binding.span);
                    if let Some(annotation) = &statement.annotation {
                        dumper.line("Annotation", annotation.span);
                        dumper.indented(|dumper| {
                            dumper.line("Colon", annotation.colon_span);
                            dumper.type_syntax(&annotation.type_syntax);
                        });
                    }
                    dumper.line("InDelimiter", statement.in_span);
                    match &statement.source {
                        ForInSource::Iterable(iterable) => {
                            dumper.heading("Iterable");
                            dumper.indented(|dumper| dumper.expression(iterable));
                        }
                        ForInSource::Range(range) => {
                            dumper.line("RangeSource", range.span);
                            dumper.indented(|dumper| {
                                dumper.heading("Lower");
                                dumper.indented(|dumper| dumper.expression(&range.lower));
                                dumper.line("DotDot", range.operator_span);
                                dumper.heading("Upper");
                                dumper.indented(|dumper| dumper.expression(&range.upper));
                            });
                        }
                    }
                    dumper.line("RightParen", statement.right_paren_span);
                    dumper.block(&statement.body);
                });
            }
            Statement::Block(block) => self.block(block),
            Statement::FieldAssignment(statement) => {
                self.line("FieldAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.heading("Place");
                    dumper.indented(|dumper| dumper.member_access(&statement.place));
                    dumper.line("Equal", statement.equal_span);
                    dumper.heading("Value");
                    dumper.indented(|dumper| dumper.expression(&statement.value));
                });
            }
            Statement::ObjectAssignment(statement) => {
                self.line("ObjectAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.heading("Place");
                    dumper.indented(|dumper| dumper.expression(&statement.place));
                    dumper.line("Equal", statement.equal_span);
                    dumper.heading("Value");
                    dumper.indented(|dumper| dumper.expression(&statement.value));
                });
            }
        }
    }

    fn conditional(&mut self, statement: &ConditionalStatement) {
        self.line("Conditional", statement.span);
        self.indented(|dumper| {
            dumper.conditional_arm("IfArm", &statement.if_arm);
            for arm in &statement.elif_arms {
                dumper.conditional_arm("ElifArm", arm);
            }
            if let Some(block) = &statement.else_block {
                dumper.heading("ElseArm");
                dumper.indented(|dumper| dumper.block(block));
            }
        });
    }

    fn conditional_arm(&mut self, name: &str, arm: &ConditionalArm) {
        self.line(name, arm.span);
        self.indented(|dumper| {
            dumper.heading("Condition");
            dumper.indented(|dumper| dumper.expression(&arm.condition));
            dumper.block(&arm.body);
        });
    }

    fn expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Absent(absent) => self.line("Absent", absent.span),
            Expression::Present(present) => {
                self.line("Present", present.span);
                self.indented(|dumper| {
                    dumper.line("Some", present.some_span);
                    dumper.expression(&present.value);
                });
            }
            Expression::Identifier(identifier) => {
                self.named("Identifier", &identifier.name.text, identifier.span);
            }
            Expression::GenericTypeApplication(application) => {
                self.named_type("GenericTypeApplication", &application.target);
            }
            Expression::GenericStaticSelection(selection) => {
                self.line("GenericStaticSelection", selection.span);
                self.indented(|dumper| {
                    dumper.named_type("Target", &selection.target);
                    dumper.line("Separator", selection.separator_span);
                    dumper.named("Member", &selection.member.text, selection.member.span);
                });
            }
            Expression::NumericLiteral(literal) => {
                let name = match literal.kind {
                    crate::literal::NumericLiteralKind::I64(_) => "Integer",
                    crate::literal::NumericLiteralKind::U64(_) => "U64",
                    crate::literal::NumericLiteralKind::U8(_) => "U8",
                    crate::literal::NumericLiteralKind::F64 => "F64",
                };
                self.named(name, &literal.spelling, literal.span);
            }
            Expression::ByteLiteral(literal) => {
                self.line(&format!("Byte {:02x}", literal.value), literal.span);
            }
            Expression::StringLiteral(literal) => {
                let mut bytes = String::with_capacity(literal.bytes.len() * 2);
                for byte in &literal.bytes {
                    let _ = write!(bytes, "{byte:02x}");
                }
                self.named("StringBytes", &bytes, literal.span);
            }
            Expression::Boolean(boolean) => {
                self.line(
                    if boolean.value {
                        "Boolean true"
                    } else {
                        "Boolean false"
                    },
                    boolean.span,
                );
            }
            Expression::Unary(unary) => {
                let operator = match unary.operator {
                    UnaryOperator::Negate => "Negate",
                    UnaryOperator::LogicalNot => "LogicalNot",
                    UnaryOperator::BitwiseComplement => "BitwiseComplement",
                    UnaryOperator::Dereference => "Dereference",
                };
                self.line(&format!("Unary {operator}"), unary.span);
                self.indented(|dumper| dumper.expression(&unary.operand));
            }
            Expression::Binary(binary) => {
                let operator = match binary.operator {
                    BinaryOperator::Add => "Add",
                    BinaryOperator::Subtract => "Subtract",
                    BinaryOperator::Multiply => "Multiply",
                    BinaryOperator::Divide => "Divide",
                    BinaryOperator::Remainder => "Remainder",
                    BinaryOperator::ShiftLeft => "ShiftLeft",
                    BinaryOperator::ShiftRight => "ShiftRight",
                    BinaryOperator::BitwiseAnd => "BitwiseAnd",
                    BinaryOperator::BitwiseOr => "BitwiseOr",
                    BinaryOperator::BitwiseXor => "BitwiseXor",
                    BinaryOperator::Equal => "Equal",
                    BinaryOperator::NotEqual => "NotEqual",
                    BinaryOperator::LessThan => "LessThan",
                    BinaryOperator::LessEqual => "LessEqual",
                    BinaryOperator::GreaterThan => "GreaterThan",
                    BinaryOperator::GreaterEqual => "GreaterEqual",
                };
                self.line(&format!("Binary {operator}"), binary.span);
                self.indented(|dumper| {
                    dumper.expression(&binary.left);
                    dumper.expression(&binary.right);
                });
            }
            Expression::Logical(logical) => {
                let operator = match logical.operator {
                    LogicalOperator::And => "And",
                    LogicalOperator::Or => "Or",
                };
                self.line(&format!("Logical {operator}"), logical.span);
                self.indented(|dumper| {
                    dumper.expression(&logical.left);
                    dumper.expression(&logical.right);
                });
            }
            Expression::TypeTest(test) => {
                self.line("TypeTest", test.span);
                self.indented(|dumper| {
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&test.source));
                    dumper.line("Is", test.is_span);
                    dumper.named_type("Target", &test.target);
                });
            }
            Expression::PresenceTest(test) => {
                let state = match test.kind {
                    PresenceTestKind::Some => "Some",
                    PresenceTestKind::None => "None",
                };
                self.line(&format!("PresenceTest {state}"), test.span);
                self.indented(|dumper| {
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&test.source));
                    dumper.line("Is", test.is_span);
                    dumper.line(state, test.target_span);
                });
            }
            Expression::Unwrap(unwrap) => {
                self.line("Unwrap", unwrap.span);
                self.indented(|dumper| {
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&unwrap.source));
                    dumper.line("Bang", unwrap.bang_span);
                });
            }
            Expression::PrimitiveCast(cast) => {
                self.line(
                    &format!("PrimitiveCast target {}", cast.target.name()),
                    cast.span,
                );
                self.indented(|dumper| {
                    dumper.line("Target", cast.target_span);
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&cast.source));
                });
            }
            Expression::ObjectCast(cast) => {
                let mode = match cast.target_mode {
                    ObjectCastTargetMode::Plain => "ObjectCast",
                    ObjectCastTargetMode::Shared { .. } => "SharedObjectCast",
                };
                self.line(mode, cast.span);
                self.indented(|dumper| {
                    dumper.named_type("Target", &cast.target);
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&cast.source));
                });
            }
            Expression::Allocation(allocation) => {
                self.line("Allocation", allocation.span);
                self.indented(|dumper| {
                    dumper.line("New", allocation.new_span);
                    dumper.named_type("Target", &allocation.target);
                    match &allocation.arguments {
                        CallArguments::Ordinary(arguments) => {
                            dumper.heading("Arguments");
                            dumper.indented(|dumper| {
                                for argument in arguments {
                                    dumper.expression(argument);
                                }
                            });
                        }
                        CallArguments::Copy { copy_span, source } => {
                            dumper.line("Copy", *copy_span);
                            dumper.heading("Source");
                            dumper.indented(|dumper| dumper.expression(source));
                        }
                    }
                });
            }
            Expression::OptionalBoxAllocation(allocation) => {
                self.line("OptionalBoxAllocation", allocation.span);
                self.indented(|dumper| {
                    dumper.line("New", allocation.new_span);
                    dumper.heading("Target");
                    dumper.indented(|dumper| dumper.type_syntax(&allocation.target));
                    match &allocation.initializer {
                        OptionalBoxInitializer::Absent {
                            left_paren_span,
                            right_paren_span,
                        } => {
                            dumper.line("LeftParen", *left_paren_span);
                            dumper.line("RightParen", *right_paren_span);
                        }
                        OptionalBoxInitializer::Value {
                            left_paren_span,
                            value,
                            right_paren_span,
                        } => {
                            dumper.line("LeftParen", *left_paren_span);
                            dumper.heading("Initializer");
                            dumper.indented(|dumper| dumper.expression(value));
                            dumper.line("RightParen", *right_paren_span);
                        }
                    }
                });
            }
            Expression::ArrayConstruction(construction) => {
                self.line(
                    if construction.new_span.is_some() {
                        "ArrayConstruction Shared"
                    } else {
                        "ArrayConstruction Inline"
                    },
                    construction.span,
                );
                self.indented(|dumper| {
                    if let Some(new_span) = construction.new_span {
                        dumper.line("New", new_span);
                    }
                    dumper.type_syntax(&construction.array_type);
                    match &construction.arguments {
                        ArrayConstructionArguments::Empty {
                            left_paren_span,
                            right_paren_span,
                        } => {
                            dumper.line("Arguments Empty", *left_paren_span);
                            dumper.line("RightParen", *right_paren_span);
                        }
                        ArrayConstructionArguments::Length {
                            left_paren_span,
                            length,
                            right_paren_span,
                        } => {
                            dumper.line("Arguments Length", *left_paren_span);
                            dumper.indented(|dumper| dumper.expression(length));
                            dumper.line("RightParen", *right_paren_span);
                        }
                        ArrayConstructionArguments::Copy {
                            left_paren_span,
                            copy_span,
                            source,
                            right_paren_span,
                        } => {
                            dumper.line("Arguments Copy", *left_paren_span);
                            dumper.line("Copy", *copy_span);
                            dumper.indented(|dumper| dumper.expression(source));
                            dumper.line("RightParen", *right_paren_span);
                        }
                        ArrayConstructionArguments::Elements(list) => {
                            dumper.line("Elements", list.left_brace_span);
                            dumper.indented(|dumper| {
                                for (index, element) in list.elements.iter().enumerate() {
                                    dumper.expression(element);
                                    if let Some(comma_span) = list.comma_spans.get(index) {
                                        dumper.line("Comma", *comma_span);
                                    }
                                }
                            });
                            dumper.line("RightBrace", list.right_brace_span);
                        }
                    }
                });
            }
            Expression::Call(call) => {
                self.line("Call", call.span);
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    match &call.arguments {
                        CallArguments::Ordinary(arguments) => {
                            dumper.heading("Arguments");
                            dumper.indented(|dumper| {
                                for argument in arguments {
                                    dumper.expression(argument);
                                }
                            });
                        }
                        CallArguments::Copy { copy_span, source } => {
                            dumper.line("Copy", *copy_span);
                            dumper.heading("Source");
                            dumper.indented(|dumper| dumper.expression(source));
                        }
                    }
                });
            }
            Expression::Grouped(grouped) => {
                self.line("Grouped", grouped.span);
                self.indented(|dumper| dumper.expression(&grouped.expression));
            }
            Expression::SelfValue(self_value) => self.line("Self", self_value.span),
            Expression::MemberAccess(member) => self.member_access(member),
            Expression::BracketProjection(projection) => self.bracket_projection(projection),
        }
    }

    fn bracket_projection(&mut self, projection: &BracketProjectionExpr) {
        self.line("BracketProjection", projection.span);
        self.indented(|dumper| {
            dumper.heading("Receiver");
            dumper.indented(|dumper| dumper.expression(&projection.receiver));
            match projection.operator {
                BracketProjectionOperator::Ordinary { left_bracket_span } => {
                    dumper.line("LeftBracket", left_bracket_span);
                }
                BracketProjectionOperator::Shared {
                    arrow_span,
                    left_bracket_span,
                } => {
                    dumper.line("SharedArrow", arrow_span);
                    dumper.line("LeftBracket", left_bracket_span);
                }
            }
            match &projection.bounds {
                BracketProjectionBounds::Index(index) => {
                    dumper.heading("Index");
                    dumper.indented(|dumper| dumper.expression(index));
                }
                BracketProjectionBounds::Slice {
                    start,
                    colon_span,
                    end,
                } => {
                    dumper.heading("Slice");
                    dumper.indented(|dumper| {
                        if let Some(start) = start {
                            dumper.heading("Start");
                            dumper.indented(|dumper| dumper.expression(start));
                        }
                        dumper.line("Colon", *colon_span);
                        if let Some(end) = end {
                            dumper.heading("End");
                            dumper.indented(|dumper| dumper.expression(end));
                        }
                    });
                }
            }
            dumper.line("RightBracket", projection.right_bracket_span);
        });
    }

    fn member_access(&mut self, member: &MemberAccessExpr) {
        self.line("MemberAccess", member.span);
        self.indented(|dumper| {
            dumper.heading("Receiver");
            dumper.indented(|dumper| dumper.expression(&member.receiver));
            let (operator, span) = match member.operator {
                MemberAccessOperator::Dot { span } => ("Dot", span),
                MemberAccessOperator::Arrow { span } => ("Arrow", span),
            };
            dumper.line(operator, span);
            dumper.named("Member", &member.member.text, member.member.span);
        });
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn named(&mut self, kind: &str, text: &str, span: Span) {
        self.write_indentation();
        let _ = write!(self.output, "{kind} ");
        write_quoted(&mut self.output, text);
        write_span(&mut self.output, span);
        self.output.push('\n');
    }

    fn line(&mut self, kind: &str, span: Span) {
        self.write_indentation();
        self.output.push_str(kind);
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
