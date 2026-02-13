from __future__ import annotations

from dataclasses import dataclass
from typing import List, Optional

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
    If,
    Index,
    IntLit,
    NamedType,
    NullLit,
    Param,
    Program,
    PtrType,
    Return,
    Span,
    StructDecl,
    StructField,
    StructFieldInit,
    StructLit,
    Sizeof,
    TypeAst,
    Unary,
    Var,
    VarDecl,
    While,
)
from tokens import Token, TokenKind


class ParseError(Exception):
    pass


@dataclass
class TokenStream:
    tokens: List[Token]
    index: int = 0

    def peek(self) -> Token:
        return self.tokens[self.index]

    def peek_n(self, n: int) -> Token:
        i = self.index + n
        if i >= len(self.tokens):
            return self.tokens[-1]
        return self.tokens[i]

    def advance(self) -> Token:
        tok = self.tokens[self.index]
        self.index += 1
        return tok

    def match(self, kind: TokenKind) -> Optional[Token]:
        if self.peek().kind == kind:
            return self.advance()
        return None

    def consume(self, kind: TokenKind, message: str) -> Token:
        tok = self.peek()
        if tok.kind != kind:
            raise ParseError(f"{message} at {tok.line}:{tok.col}")
        return self.advance()


class Parser:
    def __init__(self, tokens: List[Token]) -> None:
        self.ts = TokenStream(tokens)

    def parse_program(self) -> Program:
        decls = []
        while self.ts.peek().kind != TokenKind.EOF:
            decls.append(self._parse_decl())
        return Program(decls)

    def _parse_decl(self):
        if self.ts.match(TokenKind.KW_STRUCT):
            return self._parse_struct_decl()
        if self.ts.match(TokenKind.KW_EXTERN):
            return self._parse_extern_fn_decl()
        if self.ts.match(TokenKind.KW_FN):
            return self._parse_fn_decl()
        tok = self.ts.peek()
        raise ParseError(f"Expected declaration at {tok.line}:{tok.col}")

    def _parse_struct_decl(self) -> StructDecl:
        start = self._span_here()
        name = self._consume_ident("Expected struct name")
        self.ts.consume(TokenKind.LBRACE, "Expected '{' after struct name")
        fields = []
        while self.ts.peek().kind != TokenKind.RBRACE:
            field_span = self._span_here()
            field_name = self._consume_ident("Expected field name")
            self.ts.consume(TokenKind.COLON, "Expected ':' after field name")
            field_type = self._parse_type()
            self.ts.consume(TokenKind.SEMI, "Expected ';' after field")
            fields.append(StructField(field_name, field_type, field_span))
        self.ts.consume(TokenKind.RBRACE, "Expected '}' after struct fields")
        return StructDecl(name, fields, start)

    def _parse_extern_fn_decl(self) -> ExternFnDecl:
        start = self._span_here()
        self.ts.consume(TokenKind.KW_FN, "Expected 'fn' after 'extern'")
        name = self._consume_ident("Expected function name")
        params = self._parse_param_list()
        self.ts.consume(TokenKind.ARROW, "Expected '->' before return type")
        ret = self._parse_type()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after extern declaration")
        return ExternFnDecl(name, params, ret, start)

    def _parse_fn_decl(self) -> FnDecl:
        start = self._span_here()
        name = self._consume_ident("Expected function name")
        params = self._parse_param_list()
        self.ts.consume(TokenKind.ARROW, "Expected '->' before return type")
        ret = self._parse_type()
        body = self._parse_block()
        return FnDecl(name, params, ret, body, start)

    def _parse_param_list(self) -> List[Param]:
        self.ts.consume(TokenKind.LPAREN, "Expected '(' before parameter list")
        params: List[Param] = []
        if self.ts.peek().kind != TokenKind.RPAREN:
            while True:
                span = self._span_here()
                param_name = self._consume_ident("Expected parameter name")
                self.ts.consume(TokenKind.COLON, "Expected ':' after parameter name")
                param_type = self._parse_type()
                params.append(Param(param_name, param_type, span))
                if not self.ts.match(TokenKind.COMMA):
                    break
        self.ts.consume(TokenKind.RPAREN, "Expected ')' after parameter list")
        return params

    def _parse_type(self) -> TypeAst:
        if self.ts.match(TokenKind.STAR):
            span = self._span_from_last()
            inner = self._parse_type()
            return PtrType(inner, span)
        span = self._span_here()
        name = self._consume_ident("Expected type name")
        return NamedType(name, span)

    def _parse_block(self) -> Block:
        self.ts.consume(TokenKind.LBRACE, "Expected '{' to start block")
        span = self._span_from_last()
        stmts = []
        while self.ts.peek().kind != TokenKind.RBRACE:
            stmts.append(self._parse_stmt())
        self.ts.consume(TokenKind.RBRACE, "Expected '}' to end block")
        return Block(stmts, span)

    def _parse_stmt(self):
        tok = self.ts.peek()
        if tok.kind == TokenKind.LBRACE:
            return self._parse_block()
        if self.ts.match(TokenKind.KW_VAR):
            return self._parse_var_decl()
        if self.ts.match(TokenKind.KW_DEFER):
            span = self._span_from_last()
            call = self._parse_defer_call()
            self.ts.consume(TokenKind.SEMI, "Expected ';' after defer call")
            return DeferCall(call, span)
        if self.ts.match(TokenKind.KW_IF):
            return self._parse_if()
        if self.ts.match(TokenKind.KW_WHILE):
            return self._parse_while()
        if self.ts.match(TokenKind.KW_RETURN):
            return self._parse_return()

        expr = self._parse_expression()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after expression")
        return ExprStmt(expr, expr.span)

    def _parse_var_decl(self) -> VarDecl:
        span = self._span_from_last()
        name = self._consume_ident("Expected variable name")
        self.ts.consume(TokenKind.COLON, "Expected ':' after variable name")
        type_ast = self._parse_type()
        self.ts.consume(TokenKind.EQ, "Expected '=' after variable type")
        init = self._parse_expression()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after variable declaration")
        return VarDecl(name, type_ast, init, span)

    def _parse_if(self) -> If:
        span = self._span_from_last()
        cond = self._parse_expression()
        then_block = self._parse_block()
        else_block = None
        if self.ts.match(TokenKind.KW_ELSE):
            else_block = self._parse_block()
        return If(cond, then_block, else_block, span)

    def _parse_while(self) -> While:
        span = self._span_from_last()
        cond = self._parse_expression()
        body = self._parse_block()
        return While(cond, body, span)

    def _parse_return(self) -> Return:
        span = self._span_from_last()
        if self.ts.peek().kind == TokenKind.SEMI:
            self.ts.advance()
            return Return(None, span)
        value = self._parse_expression()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after return value")
        return Return(value, span)

    def _parse_expression(self) -> Expr:
        return self._parse_assignment()

    def _parse_assignment(self) -> Expr:
        expr = self._parse_logic_or()
        if self.ts.match(TokenKind.EQ):
            span = self._span_from_last()
            value = self._parse_assignment()
            if not self._is_lvalue(expr):
                raise ParseError("Invalid assignment target")
            return Assign(expr, value, span)
        return expr

    def _parse_logic_or(self) -> Expr:
        expr = self._parse_logic_and()
        while self.ts.match(TokenKind.OROR):
            span = self._span_from_last()
            right = self._parse_logic_and()
            expr = Binary("||", expr, right, span)
        return expr

    def _parse_logic_and(self) -> Expr:
        expr = self._parse_equality()
        while self.ts.match(TokenKind.ANDAND):
            span = self._span_from_last()
            right = self._parse_equality()
            expr = Binary("&&", expr, right, span)
        return expr

    def _parse_equality(self) -> Expr:
        expr = self._parse_relational()
        while True:
            if self.ts.match(TokenKind.EQEQ):
                span = self._span_from_last()
                right = self._parse_relational()
                expr = Binary("==", expr, right, span)
            elif self.ts.match(TokenKind.BANGEQ):
                span = self._span_from_last()
                right = self._parse_relational()
                expr = Binary("!=", expr, right, span)
            else:
                return expr

    def _parse_relational(self) -> Expr:
        expr = self._parse_additive()
        while True:
            if self.ts.match(TokenKind.LT):
                span = self._span_from_last()
                right = self._parse_additive()
                expr = Binary("<", expr, right, span)
            elif self.ts.match(TokenKind.LTE):
                span = self._span_from_last()
                right = self._parse_additive()
                expr = Binary("<=", expr, right, span)
            elif self.ts.match(TokenKind.GT):
                span = self._span_from_last()
                right = self._parse_additive()
                expr = Binary(">", expr, right, span)
            elif self.ts.match(TokenKind.GTE):
                span = self._span_from_last()
                right = self._parse_additive()
                expr = Binary(">=", expr, right, span)
            else:
                return expr

    def _parse_additive(self) -> Expr:
        expr = self._parse_multiplicative()
        while True:
            if self.ts.match(TokenKind.PLUS):
                span = self._span_from_last()
                right = self._parse_multiplicative()
                expr = Binary("+", expr, right, span)
            elif self.ts.match(TokenKind.MINUS):
                span = self._span_from_last()
                right = self._parse_multiplicative()
                expr = Binary("-", expr, right, span)
            else:
                return expr

    def _parse_multiplicative(self) -> Expr:
        expr = self._parse_unary()
        while True:
            if self.ts.match(TokenKind.STAR):
                span = self._span_from_last()
                right = self._parse_unary()
                expr = Binary("*", expr, right, span)
            elif self.ts.match(TokenKind.SLASH):
                span = self._span_from_last()
                right = self._parse_unary()
                expr = Binary("/", expr, right, span)
            elif self.ts.match(TokenKind.PERCENT):
                span = self._span_from_last()
                right = self._parse_unary()
                expr = Binary("%", expr, right, span)
            else:
                return expr

    def _parse_unary(self) -> Expr:
        if self.ts.match(TokenKind.MINUS):
            span = self._span_from_last()
            return Unary("-", self._parse_unary(), span)
        if self.ts.match(TokenKind.BANG):
            span = self._span_from_last()
            return Unary("!", self._parse_unary(), span)
        if self.ts.match(TokenKind.STAR):
            span = self._span_from_last()
            return Unary("*", self._parse_unary(), span)
        if self.ts.match(TokenKind.AMP):
            span = self._span_from_last()
            return Unary("&", self._parse_unary(), span)
        return self._parse_postfix()

    def _parse_postfix(self) -> Expr:
        expr = self._parse_primary()
        while True:
            if self.ts.match(TokenKind.LPAREN):
                span = expr.span
                args: List[Expr] = []
                if self.ts.peek().kind != TokenKind.RPAREN:
                    while True:
                        args.append(self._parse_expression())
                        if not self.ts.match(TokenKind.COMMA):
                            break
                self.ts.consume(TokenKind.RPAREN, "Expected ')' after arguments")
                expr = Call(expr, args, span)
                continue
            if self.ts.match(TokenKind.DOT):
                span = self._span_from_last()
                name = self._consume_ident("Expected field name after '.'")
                expr = Field(expr, name, span)
                continue
            if self.ts.match(TokenKind.LBRACKET):
                span = self._span_from_last()
                idx = self._parse_expression()
                self.ts.consume(TokenKind.RBRACKET, "Expected ']' after index")
                expr = Index(expr, idx, span)
                continue
            break
        return expr

    def _parse_primary(self) -> Expr:
        tok = self.ts.peek()
        if self.ts.match(TokenKind.INT):
            span = self._span_from_token(tok)
            return IntLit(int(tok.lexeme), span)
        if self.ts.match(TokenKind.KW_TRUE):
            span = self._span_from_token(tok)
            return BoolLit(True, span)
        if self.ts.match(TokenKind.KW_FALSE):
            span = self._span_from_token(tok)
            return BoolLit(False, span)
        if self.ts.match(TokenKind.KW_NULL):
            span = self._span_from_token(tok)
            return NullLit(span)
        if self.ts.match(TokenKind.KW_SIZEOF):
            span = self._span_from_last()
            self.ts.consume(TokenKind.LPAREN, "Expected '(' after sizeof")
            type_ast = self._parse_type()
            self.ts.consume(TokenKind.RPAREN, "Expected ')' after sizeof type")
            return Sizeof(type_ast, span)
        if self.ts.match(TokenKind.IDENT):
            span = self._span_from_token(tok)
            if self._starts_struct_lit():
                return self._parse_struct_lit(tok.lexeme, span)
            return Var(tok.lexeme, span)
        if self.ts.match(TokenKind.LPAREN):
            expr = self._parse_expression()
            self.ts.consume(TokenKind.RPAREN, "Expected ')' after expression")
            return expr
        raise ParseError(f"Unexpected token {tok.kind} at {tok.line}:{tok.col}")

    def _parse_struct_lit(self, type_name: str, span: Span) -> StructLit:
        self.ts.consume(TokenKind.LBRACE, "Expected '{' in struct literal")
        fields: List[StructFieldInit] = []
        if self.ts.peek().kind != TokenKind.RBRACE:
            while True:
                field_span = self._span_here()
                field_name = self._consume_ident("Expected field name in struct literal")
                self.ts.consume(TokenKind.COLON, "Expected ':' after struct literal field")
                field_value = self._parse_expression()
                fields.append(StructFieldInit(field_name, field_value, field_span))
                if not self.ts.match(TokenKind.COMMA):
                    break
                if self.ts.peek().kind == TokenKind.RBRACE:
                    break
        self.ts.consume(TokenKind.RBRACE, "Expected '}' after struct literal")
        return StructLit(type_name, fields, span)

    def _starts_struct_lit(self) -> bool:
        if self.ts.peek().kind != TokenKind.LBRACE:
            return False
        first = self.ts.peek_n(1)
        second = self.ts.peek_n(2)
        return first.kind == TokenKind.IDENT and second.kind == TokenKind.COLON

    def _parse_defer_call(self) -> Call:
        expr = self._parse_expression()
        if not isinstance(expr, Call):
            raise ParseError("defer requires a call expression")
        return expr

    def _span_here(self) -> Span:
        return self._span_from_token(self.ts.peek())

    def _span_from_last(self) -> Span:
        return self._span_from_token(self.ts.tokens[self.ts.index - 1])

    def _span_from_token(self, tok: Token) -> Span:
        return Span(tok.filepath, tok.line, tok.col)

    def _is_lvalue(self, expr: Expr) -> bool:
        return isinstance(expr, (Var, Field, Index)) or (
            isinstance(expr, Unary) and expr.op == "*"
        )

    def _consume_ident(self, message: str) -> str:
        tok = self.ts.peek()
        if tok.kind != TokenKind.IDENT:
            raise ParseError(f"{message} at {tok.line}:{tok.col}")
        self.ts.advance()
        return tok.lexeme
