from __future__ import annotations

from dataclasses import dataclass
from typing import List, Optional

from ast_nodes import (
    Assign,
    Block,
    BoolLit,
    Call,
    DeferCall,
    Expr,
    ExprStmt,
    ExternFnDecl,
    FnDecl,
    Goto,
    If,
    IntLit,
    LabeledBlock,
    NamedType,
    NullLit,
    Program,
    PtrType,
    Return,
    Span,
    StructDecl,
    TypeAst,
    Var,
    VarDecl,
    While,
)
from symbols import GlobalSymbols


def lower_program(program: Program, symbols: GlobalSymbols) -> Program:
    return Lowerer(symbols).lower_program(program)


@dataclass
class Lowerer:
    symbols: GlobalSymbols
    temp_id: int = 0

    def lower_program(self, program: Program) -> Program:
        decls = []
        for decl in program.decls:
            if isinstance(decl, FnDecl):
                decls.append(self._lower_fn(decl))
            elif isinstance(decl, (StructDecl, ExternFnDecl)):
                decls.append(decl)
            else:
                raise ValueError(f"Unsupported declaration: {type(decl)}")
        return Program(decls)

    def _lower_fn(self, fn: FnDecl) -> FnDecl:
        exit_label = f"__fn_exit_{fn.name}"
        ret_var = None if _is_unit_type(fn.ret) else f"__ret_{fn.name}"

        lowered_body = self._lower_block(fn.body, ret_var, exit_label, {})
        stmts = []
        if ret_var is not None:
            init = _default_value_expr(fn.ret, fn.span)
            stmts.append(VarDecl(ret_var, fn.ret, init, fn.span))
        stmts.extend(lowered_body.stmts)
        exit_span = fn.span
        exit_stmts = (
            [Return(Var(ret_var, exit_span), exit_span)]
            if ret_var is not None
            else [Return(None, exit_span)]
        )
        stmts.append(LabeledBlock(exit_label, Block(exit_stmts, exit_span), exit_span))

        return FnDecl(fn.name, fn.params, fn.ret, Block(stmts, fn.span), fn.span)

    def _lower_block(
        self,
        block: Block,
        ret_var: Optional[str],
        exit_label: str,
        env: dict[str, TypeAst],
    ) -> Block:
        stmts = []
        for stmt in block.stmts:
            stmts.extend(self._lower_stmt(stmt, ret_var, exit_label, env))
        return Block(stmts, block.span)

    def _lower_stmt(
        self,
        stmt,
        ret_var: Optional[str],
        exit_label: str,
        env: dict[str, TypeAst],
    ) -> List:
        if isinstance(stmt, Block):
            inner_env = env.copy()
            return [self._lower_block(stmt, ret_var, exit_label, inner_env)]
        if isinstance(stmt, VarDecl):
            env[stmt.name] = stmt.type_ast
            return [stmt]
        if isinstance(stmt, DeferCall):
            return self._lower_defer_call(stmt, env)
        if isinstance(stmt, If):
            then_block = self._lower_block(
                stmt.then_block, ret_var, exit_label, env.copy()
            )
            else_block = (
                self._lower_block(stmt.else_block, ret_var, exit_label, env.copy())
                if stmt.else_block is not None
                else None
            )
            return [If(stmt.cond, then_block, else_block, stmt.span)]
        if isinstance(stmt, While):
            body = self._lower_block(stmt.body, ret_var, exit_label, env.copy())
            return [While(stmt.cond, body, stmt.span)]
        if isinstance(stmt, Return):
            lowered = []
            if stmt.value is not None:
                if ret_var is None:
                    raise ValueError("Return value in unit function")
                lowered.append(
                    ExprStmt(
                        Assign(Var(ret_var, stmt.span), stmt.value, stmt.span),
                        stmt.span,
                    )
                )
            lowered.append(Goto(exit_label, stmt.span))
            return lowered
        if isinstance(stmt, ExprStmt):
            return [stmt]
        if isinstance(stmt, (Goto, LabeledBlock)):
            return [stmt]
        raise ValueError(f"Unsupported statement: {type(stmt)}")

    def _lower_defer_call(
        self,
        stmt: DeferCall,
        env: dict[str, TypeAst],
    ) -> List:
        call = stmt.call
        if not isinstance(call.callee, Var):
            raise ValueError("defer requires a named function call")
        fn = self.symbols.functions.get(call.callee.name)
        if fn is None:
            raise ValueError(f"Unknown function in defer: {call.callee.name}")
        if len(call.args) != len(fn.params):
            raise ValueError(f"defer arg count mismatch for {fn.name}")

        lowered_stmts: List = []
        temp_args: List[Expr] = []
        for arg, param in zip(call.args, fn.params, strict=True):
            temp_name = f"__defer_{fn.name}_{self.temp_id}"
            self.temp_id += 1
            temp_type = param.type_ast
            env[temp_name] = temp_type
            lowered_stmts.append(VarDecl(temp_name, temp_type, arg, stmt.span))
            temp_args.append(Var(temp_name, stmt.span))

        lowered_stmts.append(DeferCall(Call(call.callee, temp_args, stmt.span), stmt.span))
        return lowered_stmts


def _default_value_expr(type_ast: TypeAst, span: Span) -> Expr:
    if isinstance(type_ast, PtrType):
        return NullLit(span)
    if isinstance(type_ast, NamedType):
        if type_ast.name in {"i64", "u64", "i32", "u32"}:
            return IntLit(0, span)
        if type_ast.name == "bool":
            return BoolLit(False, span)
        if type_ast.name == "unit":
            return NullLit(span)
        raise ValueError(f"No default value for struct return type: {type_ast.name}")
    raise ValueError(f"Unsupported return type: {type(type_ast)}")


def _is_unit_type(type_ast: TypeAst) -> bool:
    return isinstance(type_ast, NamedType) and type_ast.name == "unit"
