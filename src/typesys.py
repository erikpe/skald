from __future__ import annotations

from dataclasses import dataclass
from typing import Dict

from ast_nodes import NamedType, PtrType, TypeAst
from symbols import GlobalSymbols


class TypeSystemError(Exception):
    pass


@dataclass(frozen=True)
class TyI64:
    pass


@dataclass(frozen=True)
class TyU64:
    pass


@dataclass(frozen=True)
class TyI32:
    pass


@dataclass(frozen=True)
class TyU32:
    pass


@dataclass(frozen=True)
class TyBool:
    pass


@dataclass(frozen=True)
class TyUnit:
    pass


@dataclass(frozen=True)
class TyNull:
    pass


@dataclass(frozen=True)
class TyPtr:
    inner: Ty


@dataclass(frozen=True)
class TyStruct:
    name: str


Ty = TyI64 | TyU64 | TyI32 | TyU32 | TyBool | TyUnit | TyNull | TyPtr | TyStruct


_BUILTIN_TYPES: Dict[str, Ty] = {
    "i64": TyI64(),
    "u64": TyU64(),
    "i32": TyI32(),
    "u32": TyU32(),
    "bool": TyBool(),
    "unit": TyUnit(),
}


def resolve_type(type_ast: TypeAst, symbols: GlobalSymbols) -> Ty:
    if isinstance(type_ast, PtrType):
        inner = resolve_type(type_ast.inner, symbols)
        return TyPtr(inner)
    if isinstance(type_ast, NamedType):
        if type_ast.name in _BUILTIN_TYPES:
            return _BUILTIN_TYPES[type_ast.name]
        if type_ast.name in symbols.structs:
            return TyStruct(type_ast.name)
        raise TypeSystemError(f"Unknown type: {type_ast.name}")
    raise TypeSystemError(f"Unknown type AST: {type(type_ast)}")


def is_int(ty: Ty) -> bool:
    return isinstance(ty, (TyI64, TyU64, TyI32, TyU32))


def is_bool(ty: Ty) -> bool:
    return isinstance(ty, TyBool)


def is_ptr(ty: Ty) -> bool:
    return isinstance(ty, TyPtr)


def is_null(ty: Ty) -> bool:
    return isinstance(ty, TyNull)


def type_name(ty: Ty) -> str:
    if isinstance(ty, TyPtr):
        return f"*{type_name(ty.inner)}"
    if isinstance(ty, TyStruct):
        return ty.name
    if isinstance(ty, TyI64):
        return "i64"
    if isinstance(ty, TyU64):
        return "u64"
    if isinstance(ty, TyI32):
        return "i32"
    if isinstance(ty, TyU32):
        return "u32"
    if isinstance(ty, TyBool):
        return "bool"
    if isinstance(ty, TyUnit):
        return "unit"
    if isinstance(ty, TyNull):
        return "null"
    return "<unknown>"


def is_assignable(target: Ty, value: Ty) -> bool:
    if isinstance(value, TyNull) and isinstance(target, TyPtr):
        return True
    return target == value
