from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Optional

from ast_nodes import (
    Assign,
    Binary,
    Block,
    BoolLit,
    Call,
    DeferCall,
    Expr,
    ExprStmt,
    ExternFnDecl,
    Field,
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
    StructDecl,
    TypeAst,
    Unary,
    Var,
    VarDecl,
    While,
)
from symbols import GlobalSymbols, StructLayout, type_size_align


class CodegenError(Exception):
    pass


@dataclass
class LocalInfo:
    type_ast: TypeAst
    offset: int


class LocalEnv:
    def __init__(self, symbols: GlobalSymbols) -> None:
        self.symbols = symbols
        self._scopes: List[Dict[str, LocalInfo]] = []
        self._offset = 0

    def push(self) -> None:
        self._scopes.append({})

    def pop(self) -> None:
        if not self._scopes:
            raise CodegenError("Scope stack underflow")
        self._scopes.pop()

    def define(self, name: str, type_ast: TypeAst) -> LocalInfo:
        size, align = type_size_align(type_ast, self.symbols)
        self._offset = _align_up(self._offset + size, align)
        info = LocalInfo(type_ast=type_ast, offset=self._offset)
        self._scopes[-1][name] = info
        return info

    def lookup(self, name: str) -> LocalInfo:
        for scope in reversed(self._scopes):
            if name in scope:
                return scope[name]
        raise CodegenError(f"Unknown local: {name}")

    @property
    def frame_size(self) -> int:
        return self._offset


class FrameSizer:
    def __init__(self, symbols: GlobalSymbols) -> None:
        self.symbols = symbols
        self._offset = 0

    def allocate(self, type_ast: TypeAst) -> None:
        size, align = type_size_align(type_ast, self.symbols)
        self._offset = _align_up(self._offset + size, align)

    @property
    def size(self) -> int:
        return self._offset


class Codegen:
    def __init__(self, symbols: GlobalSymbols, sources: Dict[str, List[str]]) -> None:
        self.symbols = symbols
        self.sources = sources
        self.lines: List[str] = []
        self.label_id = 0
        self.defer_stack: List[List[Call]] = []
        self._last_loc: Optional[tuple[str, int]] = None

    def emit_program(self, program: Program) -> str:
        self.lines = []
        self._emit_line(".intel_syntax noprefix")
        self._emit_line(".text")
        self._emit_line(".section .note.GNU-stack,\"\",@progbits")
        self._emit_line(".text")
        for decl in program.decls:
            if isinstance(decl, FnDecl):
                self._emit_fn(decl)
        return "\n".join(self.lines) + "\n"

    def _emit_fn(self, fn: FnDecl) -> None:
        frame_size = self._compute_frame_size(fn)
        self._emit_line("")
        self._emit_line(f".globl {fn.name}")
        self._emit_line(f"{fn.name}:")
        self._emit_line("  push rbp")
        self._emit_line("  mov rbp, rsp")
        if frame_size > 0:
            self._emit_line(f"  sub rsp, {frame_size}")

        env = LocalEnv(self.symbols)
        env.push()
        self.defer_stack = []
        self._push_defer_scope()

        arg_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
        if len(fn.params) > len(arg_regs):
            raise CodegenError("More than 6 parameters not supported")
        for i, param in enumerate(fn.params):
            info = env.define(param.name, param.type_ast)
            self._store_from_reg(arg_regs[i], info.offset, info.type_ast)

        self._emit_block(fn.body, env)

        self._pop_defer_scope(env)
        env.pop()

    def _compute_frame_size(self, fn: FnDecl) -> int:
        sizer = FrameSizer(self.symbols)
        for param in fn.params:
            sizer.allocate(param.type_ast)
        self._size_block(fn.body, sizer)
        size = _align_up(sizer.size, 16)
        return size

    def _size_block(self, block: Block, sizer: FrameSizer) -> None:
        for stmt in block.stmts:
            self._size_stmt(stmt, sizer)

    def _size_stmt(self, stmt, sizer: FrameSizer) -> None:
        if isinstance(stmt, Block):
            self._size_block(stmt, sizer)
            return
        if isinstance(stmt, VarDecl):
            sizer.allocate(stmt.type_ast)
            return
        if isinstance(stmt, DeferCall):
            return
        if isinstance(stmt, If):
            self._size_block(stmt.then_block, sizer)
            if stmt.else_block is not None:
                self._size_block(stmt.else_block, sizer)
            return
        if isinstance(stmt, While):
            self._size_block(stmt.body, sizer)
            return
        if isinstance(stmt, LabeledBlock):
            self._size_block(stmt.block, sizer)
            return

    def _emit_block(self, block: Block, env: LocalEnv) -> None:
        env.push()
        self._push_defer_scope()
        for stmt in block.stmts:
            self._emit_stmt(stmt, env)
        self._emit_defers(self.defer_stack[-1], env)
        self._pop_defer_scope(env)
        env.pop()

    def _emit_stmt(self, stmt, env: LocalEnv) -> None:
        if isinstance(stmt, Block):
            self._emit_block(stmt, env)
            return
        if isinstance(stmt, VarDecl):
            self._emit_loc(stmt.span)
            info = env.define(stmt.name, stmt.type_ast)
            self._emit_expr(stmt.init, env)
            self._store_rax(info.offset, info.type_ast)
            return
        if isinstance(stmt, DeferCall):
            self._emit_loc(stmt.span)
            self.defer_stack[-1].append(stmt.call)
            return
        if isinstance(stmt, If):
            self._emit_loc(stmt.span)
            else_label = self._new_label(".else")
            end_label = self._new_label(".endif")
            self._emit_expr(stmt.cond, env)
            self._emit_line("  cmp rax, 0")
            self._emit_line(f"  je {else_label}")
            self._emit_block(stmt.then_block, env)
            self._emit_line(f"  jmp {end_label}")
            self._emit_line(f"{else_label}:")
            if stmt.else_block is not None:
                self._emit_block(stmt.else_block, env)
            self._emit_line(f"{end_label}:")
            return
        if isinstance(stmt, While):
            self._emit_loc(stmt.span)
            start_label = self._new_label(".while")
            end_label = self._new_label(".endwhile")
            self._emit_line(f"{start_label}:")
            self._emit_expr(stmt.cond, env)
            self._emit_line("  cmp rax, 0")
            self._emit_line(f"  je {end_label}")
            self._emit_block(stmt.body, env)
            self._emit_line(f"  jmp {start_label}")
            self._emit_line(f"{end_label}:")
            return
        if isinstance(stmt, ExprStmt):
            self._emit_loc(stmt.span)
            self._emit_expr(stmt.expr, env)
            return
        if isinstance(stmt, Goto):
            self._emit_loc(stmt.span)
            self._emit_defers_all(env)
            self._emit_line(f"  jmp {stmt.label}")
            return
        if isinstance(stmt, LabeledBlock):
            self._emit_loc(stmt.span)
            self._emit_line(f"{stmt.label}:")
            self._emit_block(stmt.block, env)
            return
        if isinstance(stmt, Return):
            self._emit_loc(stmt.span)
            if stmt.value is not None:
                self._emit_expr(stmt.value, env)
            self._emit_epilogue()
            return
        raise CodegenError(f"Unsupported statement: {type(stmt)}")

    def _emit_expr(self, expr: Expr, env: LocalEnv) -> None:
        if isinstance(expr, IntLit):
            self._emit_line(f"  mov rax, {expr.value}")
            return
        if isinstance(expr, BoolLit):
            self._emit_line(f"  mov rax, {1 if expr.value else 0}")
            return
        if isinstance(expr, NullLit):
            self._emit_line("  xor rax, rax")
            return
        if isinstance(expr, Var):
            info = env.lookup(expr.name)
            self._load_to_rax(info.offset, info.type_ast)
            return
        if isinstance(expr, Unary):
            self._emit_unary(expr, env)
            return
        if isinstance(expr, Binary):
            self._emit_binary(expr, env)
            return
        if isinstance(expr, Call):
            self._emit_call(expr, env)
            return
        if isinstance(expr, Field):
            self._emit_addr(expr, env)
            field_type = self._field_type(expr, env)
            self._load_indirect_to_rax("rax", field_type)
            return
        if isinstance(expr, Assign):
            self._emit_assign(expr, env)
            return
        raise CodegenError(f"Unsupported expression: {type(expr)}")

    def _emit_unary(self, expr: Unary, env: LocalEnv) -> None:
        if expr.op == "-":
            self._emit_expr(expr.expr, env)
            self._emit_line("  neg rax")
            return
        if expr.op == "!":
            self._emit_expr(expr.expr, env)
            self._emit_line("  cmp rax, 0")
            self._emit_line("  sete al")
            self._emit_line("  movzx rax, al")
            return
        if expr.op == "*":
            self._emit_expr(expr.expr, env)
            self._load_indirect_to_rax("rax", None)
            return
        if expr.op == "&":
            self._emit_addr(expr.expr, env)
            return
        raise CodegenError(f"Unknown unary op: {expr.op}")

    def _emit_binary(self, expr: Binary, env: LocalEnv) -> None:
        if expr.op in {"&&", "||"}:
            self._emit_short_circuit(expr, env)
            return

        self._emit_expr(expr.left, env)
        self._emit_line("  push rax")
        self._emit_expr(expr.right, env)
        self._emit_line("  pop rcx")

        if expr.op == "+":
            self._emit_line("  add rcx, rax")
            self._emit_line("  mov rax, rcx")
            return
        if expr.op == "-":
            self._emit_line("  sub rcx, rax")
            self._emit_line("  mov rax, rcx")
            return
        if expr.op == "*":
            self._emit_line("  imul rcx, rax")
            self._emit_line("  mov rax, rcx")
            return
        if expr.op == "/" or expr.op == "%":
            self._emit_line("  mov r8, rax")
            self._emit_line("  mov rax, rcx")
            self._emit_line("  cqo")
            self._emit_line("  idiv r8")
            if expr.op == "%":
                self._emit_line("  mov rax, rdx")
            return
        if expr.op in {"==", "!=", "<", "<=", ">", ">="}:
            self._emit_line("  cmp rcx, rax")
            cc = {
                "==": "e",
                "!=": "ne",
                "<": "l",
                "<=": "le",
                ">": "g",
                ">=": "ge",
            }[expr.op]
            self._emit_line(f"  set{cc} al")
            self._emit_line("  movzx rax, al")
            return
        raise CodegenError(f"Unknown binary op: {expr.op}")

    def _emit_short_circuit(self, expr: Binary, env: LocalEnv) -> None:
        if expr.op == "&&":
            false_label = self._new_label(".and_false")
            end_label = self._new_label(".and_end")
            self._emit_expr(expr.left, env)
            self._emit_line("  cmp rax, 0")
            self._emit_line(f"  je {false_label}")
            self._emit_expr(expr.right, env)
            self._emit_line("  cmp rax, 0")
            self._emit_line("  setne al")
            self._emit_line("  movzx rax, al")
            self._emit_line(f"  jmp {end_label}")
            self._emit_line(f"{false_label}:")
            self._emit_line("  xor rax, rax")
            self._emit_line(f"{end_label}:")
            return

        if expr.op == "||":
            true_label = self._new_label(".or_true")
            end_label = self._new_label(".or_end")
            self._emit_expr(expr.left, env)
            self._emit_line("  cmp rax, 0")
            self._emit_line(f"  jne {true_label}")
            self._emit_expr(expr.right, env)
            self._emit_line("  cmp rax, 0")
            self._emit_line("  setne al")
            self._emit_line("  movzx rax, al")
            self._emit_line(f"  jmp {end_label}")
            self._emit_line(f"{true_label}:")
            self._emit_line("  mov rax, 1")
            self._emit_line(f"{end_label}:")
            return

    def _emit_call(self, expr: Call, env: LocalEnv) -> None:
        if not isinstance(expr.callee, Var):
            raise CodegenError("Call target must be a function name")
        arg_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
        if len(expr.args) > len(arg_regs):
            raise CodegenError("More than 6 arguments not supported")

        for arg in expr.args:
            self._emit_expr(arg, env)
            self._emit_line("  push rax")

        for i in range(len(expr.args) - 1, -1, -1):
            self._emit_line(f"  pop {arg_regs[i]}")

        self._emit_line(f"  call {expr.callee.name}")

    def _emit_assign(self, expr: Assign, env: LocalEnv) -> None:
        self._emit_addr(expr.target, env)
        self._emit_line("  push rax")
        self._emit_expr(expr.value, env)
        self._emit_line("  pop rcx")
        target_type = self._lvalue_type(expr.target, env)
        self._store_indirect_from_rax("rcx", target_type)

    def _emit_addr(self, expr: Expr, env: LocalEnv) -> None:
        if isinstance(expr, Var):
            info = env.lookup(expr.name)
            self._emit_line(f"  lea rax, [rbp - {info.offset}]")
            return
        if isinstance(expr, Unary) and expr.op == "*":
            self._emit_expr(expr.expr, env)
            return
        if isinstance(expr, Field):
            base_type = self._base_struct_type(expr.base, env)
            layout = self._struct_layout(base_type)
            field = layout.field_map.get(expr.name)
            if field is None:
                raise CodegenError(f"Unknown field {expr.name} on {base_type}")
            self._emit_addr(expr.base, env)
            if field.offset:
                self._emit_line(f"  add rax, {field.offset}")
            return
        raise CodegenError("Expression is not addressable")

    def _load_to_rax(self, offset: int, type_ast: TypeAst) -> None:
        size, _ = type_size_align(type_ast, self.symbols)
        self._load_indirect_to_rax(f"rbp - {offset}", type_ast, base_is_addr=False)

    def _store_rax(self, offset: int, type_ast: TypeAst) -> None:
        size, _ = type_size_align(type_ast, self.symbols)
        self._store_indirect_from_rax(f"rbp - {offset}", type_ast, base_is_addr=False)

    def _load_indirect_to_rax(
        self, base: str, type_ast: Optional[TypeAst], base_is_addr: bool = True
    ) -> None:
        size = 8
        if type_ast is not None:
            size, _ = type_size_align(type_ast, self.symbols)
        if base_is_addr:
            addr = f"[{base}]"
        else:
            addr = f"[{base}]"
        if size == 1:
            self._emit_line(f"  movzx rax, byte ptr {addr}")
        elif size == 4:
            self._emit_line(f"  mov eax, dword ptr {addr}")
        else:
            self._emit_line(f"  mov rax, qword ptr {addr}")

    def _store_indirect_from_rax(
        self, base: str, type_ast: Optional[TypeAst], base_is_addr: bool = True
    ) -> None:
        size = 8
        if type_ast is not None:
            size, _ = type_size_align(type_ast, self.symbols)
        addr = f"[{base}]" if base_is_addr else f"[{base}]"
        if size == 1:
            self._emit_line(f"  mov byte ptr {addr}, al")
        elif size == 4:
            self._emit_line(f"  mov dword ptr {addr}, eax")
        else:
            self._emit_line(f"  mov qword ptr {addr}, rax")

    def _store_from_reg(self, reg: str, offset: int, type_ast: TypeAst) -> None:
        size, _ = type_size_align(type_ast, self.symbols)
        if size == 1:
            reg8 = {
                "rdi": "dil",
                "rsi": "sil",
                "rdx": "dl",
                "rcx": "cl",
                "r8": "r8b",
                "r9": "r9b",
            }[reg]
            self._emit_line(f"  mov byte ptr [rbp - {offset}], {reg8}")
        elif size == 4:
            reg32 = {
                "rdi": "edi",
                "rsi": "esi",
                "rdx": "edx",
                "rcx": "ecx",
                "r8": "r8d",
                "r9": "r9d",
            }[reg]
            self._emit_line(f"  mov dword ptr [rbp - {offset}], {reg32}")
        else:
            self._emit_line(f"  mov qword ptr [rbp - {offset}], {reg}")

    def _lvalue_type(self, expr: Expr, env: LocalEnv) -> TypeAst:
        if isinstance(expr, Var):
            return env.lookup(expr.name).type_ast
        if isinstance(expr, Field):
            base_type = self._base_struct_type(expr.base, env)
            layout = self._struct_layout(base_type)
            field = layout.field_map.get(expr.name)
            if field is None:
                raise CodegenError(f"Unknown field {expr.name} on {base_type}")
            return field.type_ast
        if isinstance(expr, Unary) and expr.op == "*":
            ptr_ty = self._expr_type(expr.expr, env)
            if isinstance(ptr_ty, PtrType):
                return ptr_ty.inner
        raise CodegenError("Cannot determine lvalue type")

    def _expr_type(self, expr: Expr, env: LocalEnv) -> TypeAst:
        if isinstance(expr, Var):
            return env.lookup(expr.name).type_ast
        if isinstance(expr, Field):
            return self._lvalue_type(expr, env)
        if isinstance(expr, Unary) and expr.op == "*":
            return self._lvalue_type(expr, env)
        raise CodegenError("Expression type unavailable")

    def _field_type(self, expr: Field, env: LocalEnv) -> TypeAst:
        base_type = self._base_struct_type(expr.base, env)
        layout = self._struct_layout(base_type)
        field = layout.field_map.get(expr.name)
        if field is None:
            raise CodegenError(f"Unknown field {expr.name} on {base_type}")
        return field.type_ast

    def _base_struct_type(self, expr: Expr, env: LocalEnv) -> str:
        if isinstance(expr, Var):
            ty = env.lookup(expr.name).type_ast
        elif isinstance(expr, Field):
            ty = self._lvalue_type(expr, env)
        else:
            raise CodegenError("Struct base must be a variable or field")
        if isinstance(ty, NamedType):
            return ty.name
        raise CodegenError("Struct base must be a struct type")

    def _struct_layout(self, name: str) -> StructLayout:
        layout = self.symbols.struct_layouts.get(name)
        if layout is None:
            raise CodegenError(f"Unknown struct: {name}")
        return layout

    def _emit_defers(self, defers: List[Call], env: LocalEnv) -> None:
        for call in reversed(defers):
            self._emit_call(call, env)

    def _emit_defers_all(self, env: LocalEnv) -> None:
        for scope in reversed(self.defer_stack):
            for call in reversed(scope):
                self._emit_call(call, env)

    def _push_defer_scope(self) -> None:
        self.defer_stack.append([])

    def _pop_defer_scope(self, env: LocalEnv) -> None:
        if not self.defer_stack:
            raise CodegenError("Defer stack underflow")
        self.defer_stack.pop()

    def _emit_epilogue(self) -> None:
        self._emit_line("  mov rsp, rbp")
        self._emit_line("  pop rbp")
        self._emit_line("  ret")

    def _new_label(self, prefix: str) -> str:
        label = f"{prefix}_{self.label_id}"
        self.label_id += 1
        return label

    def _emit_line(self, line: str) -> None:
        self.lines.append(line)

    def _emit_loc(self, span) -> None:
        if span is None:
            return
        key = (span.filepath, span.line)
        if self._last_loc == key:
            return
        self._last_loc = key
        line_text = self._source_line(span.filepath, span.line)
        if line_text is None:
            self._emit_line(f"  # {span.filepath}:{span.line}:{span.col}")
            return
        self._emit_line(
            f"  # {span.filepath}:{span.line}:{span.col} | {line_text}"
        )

    def _source_line(self, filepath: str, line: int) -> Optional[str]:
        lines = self.sources.get(filepath)
        if lines is None:
            return None
        if line <= 0 or line > len(lines):
            return None
        return lines[line - 1].rstrip("\n")


def _align_up(value: int, align: int) -> int:
    if align <= 1:
        return value
    return (value + align - 1) // align * align
