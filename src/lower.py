from __future__ import annotations

from typing import List, Optional

from ast_nodes import (
    Assign,
    Block,
    BoolLit,
    Defer,
    Expr,
    ExprStmt,
    ExternFnDecl,
    FnDecl,
    Goto,
    If,
    IntLit,
    Label,
    NamedType,
    NullLit,
    Program,
    PtrType,
    Return,
    StructDecl,
    TypeAst,
    Var,
    VarDecl,
    While,
)


def lower_program(program: Program) -> Program:
    decls = []
    for decl in program.decls:
        if isinstance(decl, FnDecl):
            decls.append(_lower_fn(decl))
        elif isinstance(decl, (StructDecl, ExternFnDecl)):
            decls.append(decl)
        else:
            raise ValueError(f"Unsupported declaration: {type(decl)}")
    return Program(decls)


def _lower_fn(fn: FnDecl) -> FnDecl:
    exit_label = f"__fn_exit_{fn.name}"
    ret_var = None if _is_unit_type(fn.ret) else f"__ret_{fn.name}"

    lowered_body = _lower_block(fn.body, ret_var, exit_label)
    stmts = []
    if ret_var is not None:
        init = _default_value_expr(fn.ret)
        stmts.append(VarDecl(ret_var, fn.ret, init))
    stmts.extend(lowered_body.stmts)
    stmts.append(Label(exit_label))
    if ret_var is not None:
        stmts.append(Return(Var(ret_var)))
    else:
        stmts.append(Return(None))

    return FnDecl(fn.name, fn.params, fn.ret, Block(stmts))


def _lower_block(block: Block, ret_var: Optional[str], exit_label: str) -> Block:
    stmts = []
    for stmt in block.stmts:
        stmts.extend(_lower_stmt(stmt, ret_var, exit_label))
    return Block(stmts)


def _lower_stmt(stmt, ret_var: Optional[str], exit_label: str) -> List:
    if isinstance(stmt, Block):
        return [_lower_block(stmt, ret_var, exit_label)]
    if isinstance(stmt, VarDecl):
        return [stmt]
    if isinstance(stmt, Defer):
        return [Defer(_lower_block(stmt.block, ret_var, exit_label))]
    if isinstance(stmt, If):
        then_block = _lower_block(stmt.then_block, ret_var, exit_label)
        else_block = (
            _lower_block(stmt.else_block, ret_var, exit_label)
            if stmt.else_block is not None
            else None
        )
        return [If(stmt.cond, then_block, else_block)]
    if isinstance(stmt, While):
        body = _lower_block(stmt.body, ret_var, exit_label)
        return [While(stmt.cond, body)]
    if isinstance(stmt, Return):
        lowered = []
        if stmt.value is not None:
            if ret_var is None:
                raise ValueError("Return value in unit function")
            lowered.append(ExprStmt(Assign(Var(ret_var), stmt.value)))
        lowered.append(Goto(exit_label))
        return lowered
    if isinstance(stmt, ExprStmt):
        return [stmt]
    if isinstance(stmt, (Label, Goto)):
        return [stmt]
    raise ValueError(f"Unsupported statement: {type(stmt)}")


def _default_value_expr(type_ast: TypeAst) -> Expr:
    if isinstance(type_ast, PtrType):
        return NullLit()
    if isinstance(type_ast, NamedType):
        if type_ast.name in {"i64", "u64", "i32", "u32"}:
            return IntLit(0)
        if type_ast.name == "bool":
            return BoolLit(False)
        if type_ast.name == "unit":
            return NullLit()
        raise ValueError(f"No default value for struct return type: {type_ast.name}")
    raise ValueError(f"Unsupported return type: {type(type_ast)}")


def _is_unit_type(type_ast: TypeAst) -> bool:
    return isinstance(type_ast, NamedType) and type_ast.name == "unit"
