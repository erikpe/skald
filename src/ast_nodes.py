from __future__ import annotations

from dataclasses import dataclass
from typing import List, Optional


@dataclass(frozen=True)
class Span:
    filepath: str
    line: int
    col: int


# Program and declarations


@dataclass(frozen=True)
class Program:
    decls: List[Decl]


@dataclass(frozen=True)
class StructField:
    name: str
    type_ast: TypeAst
    span: Span


@dataclass(frozen=True)
class StructDecl:
    name: str
    fields: List[StructField]
    span: Span


@dataclass(frozen=True)
class Param:
    name: str
    type_ast: TypeAst
    span: Span


@dataclass(frozen=True)
class FnDecl:
    name: str
    params: List[Param]
    ret: TypeAst
    body: Block
    span: Span


@dataclass(frozen=True)
class ExternFnDecl:
    name: str
    params: List[Param]
    ret: TypeAst
    span: Span


Decl = StructDecl | FnDecl | ExternFnDecl


# Types


@dataclass(frozen=True)
class NamedType:
    name: str
    span: Span


@dataclass(frozen=True)
class PtrType:
    inner: TypeAst
    span: Span


TypeAst = NamedType | PtrType


# Statements


@dataclass(frozen=True)
class Block:
    stmts: List[Stmt]
    span: Span


@dataclass(frozen=True)
class VarDecl:
    name: str
    type_ast: TypeAst
    init: Expr
    span: Span


@dataclass(frozen=True)
class DeferCall:
    call: Call
    span: Span


@dataclass(frozen=True)
class If:
    cond: Expr
    then_block: Block
    else_block: Optional[Block]
    span: Span


@dataclass(frozen=True)
class While:
    cond: Expr
    body: Block
    span: Span


@dataclass(frozen=True)
class Return:
    value: Optional[Expr]
    span: Span


@dataclass(frozen=True)
class ExprStmt:
    expr: Expr
    span: Span


@dataclass(frozen=True)
class Goto:
    label: str
    span: Span


@dataclass(frozen=True)
class LabeledBlock:
    label: str
    block: Block
    span: Span


Stmt = Block | VarDecl | DeferCall | If | While | Return | ExprStmt | Goto | LabeledBlock


# Expressions


@dataclass(frozen=True)
class IntLit:
    value: int
    span: Span


@dataclass(frozen=True)
class BoolLit:
    value: bool
    span: Span


@dataclass(frozen=True)
class NullLit:
    span: Span


@dataclass(frozen=True)
class Var:
    name: str
    span: Span


@dataclass(frozen=True)
class Unary:
    op: str
    expr: Expr
    span: Span


@dataclass(frozen=True)
class Binary:
    op: str
    left: Expr
    right: Expr
    span: Span


@dataclass(frozen=True)
class Call:
    callee: Expr
    args: List[Expr]
    span: Span


@dataclass(frozen=True)
class Field:
    base: Expr
    name: str
    span: Span


@dataclass(frozen=True)
class Assign:
    target: Expr
    value: Expr
    span: Span


Expr = (
    IntLit
    | BoolLit
    | NullLit
    | Var
    | Unary
    | Binary
    | Call
    | Field
    | Assign
)
