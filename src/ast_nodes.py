from __future__ import annotations

from dataclasses import dataclass
from typing import List, Optional


# Program and declarations


@dataclass(frozen=True)
class Program:
    decls: List[Decl]


@dataclass(frozen=True)
class StructField:
    name: str
    type_ast: TypeAst


@dataclass(frozen=True)
class StructDecl:
    name: str
    fields: List[StructField]


@dataclass(frozen=True)
class Param:
    name: str
    type_ast: TypeAst


@dataclass(frozen=True)
class FnDecl:
    name: str
    params: List[Param]
    ret: TypeAst
    body: Block


@dataclass(frozen=True)
class ExternFnDecl:
    name: str
    params: List[Param]
    ret: TypeAst


Decl = StructDecl | FnDecl | ExternFnDecl


# Types


@dataclass(frozen=True)
class NamedType:
    name: str


@dataclass(frozen=True)
class PtrType:
    inner: TypeAst


TypeAst = NamedType | PtrType


# Statements


@dataclass(frozen=True)
class Block:
    stmts: List[Stmt]


@dataclass(frozen=True)
class VarDecl:
    name: str
    type_ast: TypeAst
    init: Expr


@dataclass(frozen=True)
class DeferCall:
    call: Call


@dataclass(frozen=True)
class If:
    cond: Expr
    then_block: Block
    else_block: Optional[Block]


@dataclass(frozen=True)
class While:
    cond: Expr
    body: Block


@dataclass(frozen=True)
class Return:
    value: Optional[Expr]


@dataclass(frozen=True)
class ExprStmt:
    expr: Expr


@dataclass(frozen=True)
class Goto:
    label: str


@dataclass(frozen=True)
class LabeledBlock:
    label: str
    block: Block


Stmt = Block | VarDecl | DeferCall | If | While | Return | ExprStmt | Goto | LabeledBlock


# Expressions


@dataclass(frozen=True)
class IntLit:
    value: int


@dataclass(frozen=True)
class BoolLit:
    value: bool


@dataclass(frozen=True)
class NullLit:
    pass


@dataclass(frozen=True)
class Var:
    name: str


@dataclass(frozen=True)
class Unary:
    op: str
    expr: Expr


@dataclass(frozen=True)
class Binary:
    op: str
    left: Expr
    right: Expr


@dataclass(frozen=True)
class Call:
    callee: Expr
    args: List[Expr]


@dataclass(frozen=True)
class Field:
    base: Expr
    name: str


@dataclass(frozen=True)
class Assign:
    target: Expr
    value: Expr


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
