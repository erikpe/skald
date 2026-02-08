from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Optional

from ast_nodes import (
    Assign,
    Binary,
    Block,
    BoolLit,
    Call,
    Defer,
    Expr,
    ExprStmt,
    Field,
    FnDecl,
    Goto,
    If,
    IntLit,
    Label,
    NamedType,
    NullLit,
    Program,
    Return,
    Unary,
    Var,
    VarDecl,
    While,
)
from symbols import GlobalSymbols
from typesys import (
    Ty,
    TyBool,
    TyNull,
    TyPtr,
    TyStruct,
    TyUnit,
    is_assignable,
    is_bool,
    is_int,
    resolve_type,
    type_name,
)


class TypeCheckError(Exception):
    pass


@dataclass
class TypeEnv:
    scopes: List[Dict[str, Ty]]

    def __init__(self) -> None:
        self.scopes = []

    def push(self) -> None:
        self.scopes.append({})

    def pop(self) -> None:
        if not self.scopes:
            raise TypeCheckError("Scope stack underflow")
        self.scopes.pop()

    def define(self, name: str, ty: Ty) -> None:
        if not self.scopes:
            raise TypeCheckError("No active scope")
        scope = self.scopes[-1]
        if name in scope:
            raise TypeCheckError(f"Duplicate local symbol: {name}")
        scope[name] = ty

    def lookup(self, name: str) -> Optional[Ty]:
        for scope in reversed(self.scopes):
            if name in scope:
                return scope[name]
        return None


def typecheck_program(program: Program, symbols: GlobalSymbols) -> None:
    for decl in program.decls:
        if isinstance(decl, FnDecl):
            _typecheck_fn(decl, symbols)


def _typecheck_fn(fn: FnDecl, symbols: GlobalSymbols) -> None:
    env = TypeEnv()
    env.push()
    for param in fn.params:
        param_ty = resolve_type(param.type_ast, symbols)
        env.define(param.name, param_ty)

    ret_ty = resolve_type(fn.ret, symbols)
    _check_block(fn.body, env, symbols, ret_ty)
    env.pop()


def _check_block(block: Block, env: TypeEnv, symbols: GlobalSymbols, ret_ty: Ty) -> None:
    env.push()
    for stmt in block.stmts:
        _check_stmt(stmt, env, symbols, ret_ty)
    env.pop()


def _check_stmt(stmt, env: TypeEnv, symbols: GlobalSymbols, ret_ty: Ty) -> None:
    if isinstance(stmt, Block):
        _check_block(stmt, env, symbols, ret_ty)
        return
    if isinstance(stmt, VarDecl):
        var_ty = resolve_type(stmt.type_ast, symbols)
        init_ty = _check_expr(stmt.init, env, symbols)
        if not is_assignable(var_ty, init_ty):
            raise TypeCheckError(
                f"Type mismatch in var init: {type_name(var_ty)} = {type_name(init_ty)}"
            )
        env.define(stmt.name, var_ty)
        return
    if isinstance(stmt, Defer):
        _check_block(stmt.block, env, symbols, ret_ty)
        return
    if isinstance(stmt, If):
        cond_ty = _check_expr(stmt.cond, env, symbols)
        if not is_bool(cond_ty):
            raise TypeCheckError(
                f"If condition must be bool, got {type_name(cond_ty)}"
            )
        _check_block(stmt.then_block, env, symbols, ret_ty)
        if stmt.else_block is not None:
            _check_block(stmt.else_block, env, symbols, ret_ty)
        return
    if isinstance(stmt, While):
        cond_ty = _check_expr(stmt.cond, env, symbols)
        if not is_bool(cond_ty):
            raise TypeCheckError(
                f"While condition must be bool, got {type_name(cond_ty)}"
            )
        _check_block(stmt.body, env, symbols, ret_ty)
        return
    if isinstance(stmt, Return):
        if stmt.value is None:
            if not isinstance(ret_ty, TyUnit):
                raise TypeCheckError("Return value required")
            return
        value_ty = _check_expr(stmt.value, env, symbols)
        if not is_assignable(ret_ty, value_ty):
            raise TypeCheckError(
                f"Return type mismatch: expected {type_name(ret_ty)}, got {type_name(value_ty)}"
            )
        return
    if isinstance(stmt, ExprStmt):
        _check_expr(stmt.expr, env, symbols)
        return
    if isinstance(stmt, (Label, Goto)):
        return
    raise TypeCheckError(f"Unknown statement type: {type(stmt)}")


def _check_expr(expr: Expr, env: TypeEnv, symbols: GlobalSymbols) -> Ty:
    if isinstance(expr, IntLit):
        return resolve_type(_builtin_named("i64"), symbols)
    if isinstance(expr, BoolLit):
        return resolve_type(_builtin_named("bool"), symbols)
    if isinstance(expr, NullLit):
        return TyNull()
    if isinstance(expr, Var):
        ty = env.lookup(expr.name)
        if ty is None:
            raise TypeCheckError(f"Unknown variable: {expr.name}")
        return ty
    if isinstance(expr, Unary):
        return _check_unary(expr, env, symbols)
    if isinstance(expr, Binary):
        return _check_binary(expr, env, symbols)
    if isinstance(expr, Call):
        return _check_call(expr, env, symbols)
    if isinstance(expr, Field):
        base_ty = _check_expr(expr.base, env, symbols)
        if not isinstance(base_ty, TyStruct):
            raise TypeCheckError(
                f"Field access requires struct, got {type_name(base_ty)}"
            )
        layout = symbols.struct_layouts.get(base_ty.name)
        if layout is None or expr.name not in layout.field_map:
            raise TypeCheckError(f"Unknown field {expr.name} on {base_ty.name}")
        field = layout.field_map[expr.name]
        return resolve_type(field.type_ast, symbols)
    if isinstance(expr, Assign):
        if not _is_lvalue(expr.target):
            raise TypeCheckError("Invalid assignment target")
        target_ty = _check_expr(expr.target, env, symbols)
        value_ty = _check_expr(expr.value, env, symbols)
        if not is_assignable(target_ty, value_ty):
            raise TypeCheckError(
                f"Assignment mismatch: {type_name(target_ty)} = {type_name(value_ty)}"
            )
        return target_ty
    raise TypeCheckError(f"Unknown expression type: {type(expr)}")


def _check_unary(expr: Unary, env: TypeEnv, symbols: GlobalSymbols) -> Ty:
    inner = _check_expr(expr.expr, env, symbols)
    if expr.op == "-":
        if not is_int(inner):
            raise TypeCheckError(f"Unary '-' expects int, got {type_name(inner)}")
        return inner
    if expr.op == "!":
        if not is_bool(inner):
            raise TypeCheckError(f"Unary '!' expects bool, got {type_name(inner)}")
        return TyBool()
    if expr.op == "*":
        if not isinstance(inner, TyPtr):
            raise TypeCheckError(f"Unary '*' expects pointer, got {type_name(inner)}")
        return inner.inner
    if expr.op == "&":
        if not _is_lvalue(expr.expr):
            raise TypeCheckError("Address-of requires lvalue")
        return TyPtr(inner)
    raise TypeCheckError(f"Unknown unary operator: {expr.op}")


def _check_binary(expr: Binary, env: TypeEnv, symbols: GlobalSymbols) -> Ty:
    left = _check_expr(expr.left, env, symbols)
    right = _check_expr(expr.right, env, symbols)

    if expr.op in {"+", "-", "*", "/", "%"}:
        if not is_int(left) or not is_int(right) or left != right:
            raise TypeCheckError("Arithmetic operators require matching integer types")
        return left

    if expr.op in {"<", "<=", ">", ">="}:
        if not is_int(left) or not is_int(right) or left != right:
            raise TypeCheckError("Relational operators require matching integer types")
        return TyBool()

    if expr.op in {"==", "!="}:
        if is_assignable(left, right) or is_assignable(right, left):
            return TyBool()
        raise TypeCheckError("Equality operators require compatible types")

    if expr.op in {"&&", "||"}:
        if not is_bool(left) or not is_bool(right):
            raise TypeCheckError("Logical operators require bool operands")
        return TyBool()

    raise TypeCheckError(f"Unknown binary operator: {expr.op}")


def _check_call(expr: Call, env: TypeEnv, symbols: GlobalSymbols) -> Ty:
    if not isinstance(expr.callee, Var):
        raise TypeCheckError("Call target must be a function name")

    local = env.lookup(expr.callee.name)
    if local is not None:
        raise TypeCheckError(f"Cannot call non-function value: {expr.callee.name}")

    fn = symbols.functions.get(expr.callee.name)
    if fn is None:
        raise TypeCheckError(f"Unknown function: {expr.callee.name}")

    if len(expr.args) != len(fn.params):
        raise TypeCheckError(
            f"Argument count mismatch for {fn.name}: expected {len(fn.params)}"
        )

    for arg_expr, param in zip(expr.args, fn.params, strict=True):
        arg_ty = _check_expr(arg_expr, env, symbols)
        param_ty = resolve_type(param.type_ast, symbols)
        if not is_assignable(param_ty, arg_ty):
            raise TypeCheckError(
                f"Argument type mismatch for {fn.name}: expected {type_name(param_ty)}, got {type_name(arg_ty)}"
            )

    return resolve_type(fn.ret, symbols)


def _is_lvalue(expr: Expr) -> bool:
    return isinstance(expr, (Var, Field)) or (
        isinstance(expr, Unary) and expr.op == "*"
    )


def _builtin_named(name: str):
    return NamedType(name)
