from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Optional

from ast_nodes import (
    Decl,
    ExternFnDecl,
    FnDecl,
    NamedType,
    Param,
    Program,
    PtrType,
    StructDecl,
    StructField,
    TypeAst,
)


class SymbolError(Exception):
    pass


@dataclass(frozen=True)
class FieldLayout:
    name: str
    type_ast: TypeAst
    offset: int
    size: int
    align: int


@dataclass(frozen=True)
class StructLayout:
    name: str
    fields: List[FieldLayout]
    size: int
    align: int
    field_map: Dict[str, FieldLayout]


@dataclass(frozen=True)
class FnSig:
    name: str
    params: List[Param]
    ret: TypeAst
    extern: bool


@dataclass
class GlobalSymbols:
    structs: Dict[str, StructDecl]
    struct_layouts: Dict[str, StructLayout]
    functions: Dict[str, FnSig]


@dataclass(frozen=True)
class LocalSymbol:
    name: str
    type_ast: TypeAst
    offset: int


class ScopeStack:
    def __init__(self) -> None:
        self._scopes: List[Dict[str, LocalSymbol]] = []

    def push(self) -> None:
        self._scopes.append({})

    def pop(self) -> None:
        if not self._scopes:
            raise SymbolError("Scope stack underflow")
        self._scopes.pop()

    def define(self, sym: LocalSymbol) -> None:
        if not self._scopes:
            raise SymbolError("No active scope")
        scope = self._scopes[-1]
        if sym.name in scope:
            raise SymbolError(f"Duplicate local symbol: {sym.name}")
        scope[sym.name] = sym

    def lookup(self, name: str) -> Optional[LocalSymbol]:
        for scope in reversed(self._scopes):
            if name in scope:
                return scope[name]
        return None


_BUILTIN_SIZES = {
    "i64": (8, 8),
    "u64": (8, 8),
    "i32": (4, 4),
    "u32": (4, 4),
    "bool": (1, 1),
    "unit": (0, 1),
}


def build_global_symbols(program: Program) -> GlobalSymbols:
    structs: Dict[str, StructDecl] = {}
    functions: Dict[str, FnSig] = {}

    for decl in program.decls:
        if isinstance(decl, StructDecl):
            if decl.name in structs:
                raise SymbolError(f"Duplicate struct: {decl.name}")
            structs[decl.name] = decl
        elif isinstance(decl, FnDecl):
            if decl.name in functions:
                raise SymbolError(f"Duplicate function: {decl.name}")
            functions[decl.name] = FnSig(decl.name, decl.params, decl.ret, extern=False)
        elif isinstance(decl, ExternFnDecl):
            if decl.name in functions:
                raise SymbolError(f"Duplicate function: {decl.name}")
            functions[decl.name] = FnSig(decl.name, decl.params, decl.ret, extern=True)
        else:
            raise SymbolError(f"Unsupported declaration type: {type(decl)}")

    struct_layouts: Dict[str, StructLayout] = {}
    for name, decl in structs.items():
        _compute_struct_layout(name, decl, structs, struct_layouts, [])

    return GlobalSymbols(structs=structs, struct_layouts=struct_layouts, functions=functions)


def _compute_struct_layout(
    name: str,
    decl: StructDecl,
    structs: Dict[str, StructDecl],
    layouts: Dict[str, StructLayout],
    visiting: List[str],
) -> StructLayout:
    if name in layouts:
        return layouts[name]
    if name in visiting:
        cycle = " -> ".join(visiting + [name])
        raise SymbolError(f"Illegal recursive struct: {cycle}")

    visiting.append(name)
    offset = 0
    struct_align = 1
    field_layouts: List[FieldLayout] = []

    for field in decl.fields:
        size, align = _type_size_align(field.type_ast, structs, layouts, visiting)
        offset = _align_up(offset, align)
        field_layouts.append(FieldLayout(field.name, field.type_ast, offset, size, align))
        offset += size
        if align > struct_align:
            struct_align = align

    size = _align_up(offset, struct_align)
    field_map = {f.name: f for f in field_layouts}
    layout = StructLayout(name, field_layouts, size, struct_align, field_map)
    layouts[name] = layout
    visiting.pop()
    return layout


def _type_size_align(
    type_ast: TypeAst,
    structs: Dict[str, StructDecl],
    layouts: Dict[str, StructLayout],
    visiting: List[str],
) -> tuple[int, int]:
    if isinstance(type_ast, PtrType):
        return 8, 8
    if isinstance(type_ast, NamedType):
        if type_ast.name in _BUILTIN_SIZES:
            return _BUILTIN_SIZES[type_ast.name]
        if type_ast.name in structs:
            layout = _compute_struct_layout(
                type_ast.name, structs[type_ast.name], structs, layouts, visiting
            )
            return layout.size, layout.align
        raise SymbolError(f"Unknown type: {type_ast.name}")
    raise SymbolError(f"Unknown type AST: {type(type_ast)}")


def _align_up(value: int, align: int) -> int:
    if align <= 1:
        return value
    return (value + align - 1) // align * align
