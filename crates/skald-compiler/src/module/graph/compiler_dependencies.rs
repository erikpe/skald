//! Compiler-owned dependency evidence derived from valid parsed source.

use std::collections::BTreeMap;

use crate::{
    lexer::{Token, TokenKind},
    source::TextRange,
    syntax::{Block, ClassMember, CompilationUnit, ForInSource, Statement, TopLevelDeclaration},
};

use super::CompilerDependencyKind;

pub(super) fn collect(
    ast: &CompilationUnit,
    tokens: &[Token],
) -> BTreeMap<CompilerDependencyKind, Vec<TextRange>> {
    let mut dependencies = BTreeMap::<CompilerDependencyKind, Vec<TextRange>>::new();
    for token in tokens {
        let kind = match token.kind {
            TokenKind::StringLiteral => CompilerDependencyKind::StringLiteral,
            TokenKind::For => CompilerDependencyKind::GeneralIteration,
            _ => continue,
        };
        dependencies
            .entry(kind)
            .or_default()
            .push(token.span.range());
    }
    collect_range_sources(ast, &mut dependencies);
    dependencies
}

fn collect_range_sources(
    ast: &CompilationUnit,
    dependencies: &mut BTreeMap<CompilerDependencyKind, Vec<TextRange>>,
) {
    for declaration in &ast.declarations {
        match declaration {
            TopLevelDeclaration::Function(function) => {
                visit_block(&function.body, dependencies);
            }
            TopLevelDeclaration::Class(class) => {
                for member in &class.members {
                    let body = match member {
                        ClassMember::Initializer(member) => Some(&member.body),
                        ClassMember::CopyConstructor(member) => Some(&member.body),
                        ClassMember::CopyAssignment(member) => Some(&member.body),
                        ClassMember::Destructor(member) => Some(&member.body),
                        ClassMember::Method(member) => Some(&member.body),
                        ClassMember::Field(_) | ClassMember::StaticField(_) => None,
                    };
                    if let Some(body) = body {
                        visit_block(body, dependencies);
                    }
                }
            }
            TopLevelDeclaration::ExternalFunction(_)
            | TopLevelDeclaration::IntrinsicFunction(_)
            | TopLevelDeclaration::Interface(_) => {}
        }
    }
}

fn visit_block(block: &Block, dependencies: &mut BTreeMap<CompilerDependencyKind, Vec<TextRange>>) {
    for statement in &block.statements {
        match statement {
            Statement::Conditional(statement) => {
                visit_block(&statement.if_arm.body, dependencies);
                for arm in &statement.elif_arms {
                    visit_block(&arm.body, dependencies);
                }
                if let Some(body) = &statement.else_block {
                    visit_block(body, dependencies);
                }
            }
            Statement::While(statement) => visit_block(&statement.body, dependencies),
            Statement::ForIn(statement) => {
                if let ForInSource::Range(range) = &statement.source {
                    dependencies
                        .entry(CompilerDependencyKind::RangeForSource)
                        .or_default()
                        .push(range.operator_span.range());
                }
                visit_block(&statement.body, dependencies);
            }
            Statement::Block(block) => visit_block(block, dependencies),
            Statement::BaseInitialization(_)
            | Statement::Local(_)
            | Statement::Return(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Expression(_)
            | Statement::FieldAssignment(_)
            | Statement::ObjectAssignment(_) => {}
        }
    }
}
