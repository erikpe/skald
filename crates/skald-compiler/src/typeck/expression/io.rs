//! Type checked construction of dedicated standard-I/O HIR.

use super::*;
use crate::{
    hir::{HirArrayAliasArgument, HirCallArgument, HirExpressionKind, HirIoOperation, Type},
    intrinsic::Intrinsic,
    resolve::{ResolvedDirectCallExpr, ResolvedFunctionDeclaration},
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_io_intrinsic_call(
        &mut self,
        call: &ResolvedDirectCallExpr,
        target: &ResolvedFunctionDeclaration,
        intrinsic: Intrinsic,
    ) -> Option<HirExpression> {
        let arguments = self.check_arguments(
            &call.arguments,
            &target.parameters,
            call.callee_span,
            "I/O intrinsic",
            Some(&target.name),
            Some(target.name_span),
        )?;
        let mut arguments = arguments.into_iter();
        let operation = match intrinsic {
            Intrinsic::IoStandardHandle => HirIoOperation::StandardHandle {
                stream: value(&mut arguments),
            },
            Intrinsic::IoOpen => HirIoOperation::Open {
                path: array_alias(&mut arguments),
                mode: value(&mut arguments),
            },
            Intrinsic::IoRead => HirIoOperation::Read {
                handle: value(&mut arguments),
                destination: array_alias(&mut arguments),
                offset: value(&mut arguments),
            },
            Intrinsic::IoWrite => HirIoOperation::Write {
                handle: value(&mut arguments),
                source: array_alias(&mut arguments),
                offset: value(&mut arguments),
            },
            Intrinsic::IoClose => HirIoOperation::Close {
                handle: value(&mut arguments),
            },
            Intrinsic::Panic => unreachable!("panic is checked as a diverging call statement"),
        };
        debug_assert!(arguments.next().is_none());
        Some(HirExpression {
            kind: HirExpressionKind::Io(Box::new(operation)),
            ty: Type::I64,
            span: call.span,
        })
    }
}

fn value(arguments: &mut impl Iterator<Item = HirCallArgument>) -> HirExpression {
    match arguments.next() {
        Some(HirCallArgument::Value(value)) => value,
        _ => unreachable!("validated scalar intrinsic parameter must produce a value argument"),
    }
}

fn array_alias(arguments: &mut impl Iterator<Item = HirCallArgument>) -> HirArrayAliasArgument {
    match arguments.next() {
        Some(HirCallArgument::ArrayAlias(alias)) => alias,
        _ => unreachable!("validated array intrinsic parameter must produce an alias argument"),
    }
}
