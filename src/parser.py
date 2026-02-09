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
    IntLit,
    NamedType,
    NullLit,
    Param,
    Program,
    PtrType,
    Return,
    StructDecl,
    StructField,
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
        name = self._consume_ident("Expected struct name")
        self.ts.consume(TokenKind.LBRACE, "Expected '{' after struct name")
        fields = []
        while self.ts.peek().kind != TokenKind.RBRACE:
            field_name = self._consume_ident("Expected field name")
            self.ts.consume(TokenKind.COLON, "Expected ':' after field name")
            field_type = self._parse_type()
            self.ts.consume(TokenKind.SEMI, "Expected ';' after field")
            fields.append(StructField(field_name, field_type))
        self.ts.consume(TokenKind.RBRACE, "Expected '}' after struct fields")
        return StructDecl(name, fields)

    def _parse_extern_fn_decl(self) -> ExternFnDecl:
        self.ts.consume(TokenKind.KW_FN, "Expected 'fn' after 'extern'")
        name = self._consume_ident("Expected function name")
        params = self._parse_param_list()
        self.ts.consume(TokenKind.ARROW, "Expected '->' before return type")
        ret = self._parse_type()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after extern declaration")
        return ExternFnDecl(name, params, ret)

    def _parse_fn_decl(self) -> FnDecl:
        name = self._consume_ident("Expected function name")
        params = self._parse_param_list()
        self.ts.consume(TokenKind.ARROW, "Expected '->' before return type")
        ret = self._parse_type()
        body = self._parse_block()
        return FnDecl(name, params, ret, body)

    def _parse_param_list(self) -> List[Param]:
        self.ts.consume(TokenKind.LPAREN, "Expected '(' before parameter list")
        params: List[Param] = []
        if self.ts.peek().kind != TokenKind.RPAREN:
            while True:
                param_name = self._consume_ident("Expected parameter name")
                self.ts.consume(TokenKind.COLON, "Expected ':' after parameter name")
                param_type = self._parse_type()
                params.append(Param(param_name, param_type))
                if not self.ts.match(TokenKind.COMMA):
                    break
        self.ts.consume(TokenKind.RPAREN, "Expected ')' after parameter list")
        return params

    def _parse_type(self) -> TypeAst:
        if self.ts.match(TokenKind.STAR):
            inner = self._parse_type()
            return PtrType(inner)
        name = self._consume_ident("Expected type name")
        return NamedType(name)

    def _parse_block(self) -> Block:
        self.ts.consume(TokenKind.LBRACE, "Expected '{' to start block")
        stmts = []
        while self.ts.peek().kind != TokenKind.RBRACE:
            stmts.append(self._parse_stmt())
        self.ts.consume(TokenKind.RBRACE, "Expected '}' to end block")
        return Block(stmts)

    def _parse_stmt(self):
        tok = self.ts.peek()
        if tok.kind == TokenKind.LBRACE:
            return self._parse_block()
        if self.ts.match(TokenKind.KW_VAR):
            return self._parse_var_decl()
        if self.ts.match(TokenKind.KW_DEFER):
            call = self._parse_defer_call()
            self.ts.consume(TokenKind.SEMI, "Expected ';' after defer call")
            return DeferCall(call)
        if self.ts.match(TokenKind.KW_IF):
            return self._parse_if()
        if self.ts.match(TokenKind.KW_WHILE):
            return self._parse_while()
        if self.ts.match(TokenKind.KW_RETURN):
            return self._parse_return()

        expr = self._parse_expression()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after expression")
        return ExprStmt(expr)

    def _parse_var_decl(self) -> VarDecl:
        name = self._consume_ident("Expected variable name")
        self.ts.consume(TokenKind.COLON, "Expected ':' after variable name")
        type_ast = self._parse_type()
        self.ts.consume(TokenKind.EQ, "Expected '=' after variable type")
        init = self._parse_expression()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after variable declaration")
        return VarDecl(name, type_ast, init)

    def _parse_if(self) -> If:
        cond = self._parse_expression()
        then_block = self._parse_block()
        else_block = None
        if self.ts.match(TokenKind.KW_ELSE):
            else_block = self._parse_block()
        return If(cond, then_block, else_block)

    def _parse_while(self) -> While:
        cond = self._parse_expression()
        body = self._parse_block()
        return While(cond, body)

    def _parse_return(self) -> Return:
        if self.ts.peek().kind == TokenKind.SEMI:
            self.ts.advance()
            return Return(None)
        value = self._parse_expression()
        self.ts.consume(TokenKind.SEMI, "Expected ';' after return value")
        return Return(value)

    def _parse_expression(self) -> Expr:
        return self._parse_assignment()

    def _parse_assignment(self) -> Expr:
        expr = self._parse_logic_or()
        if self.ts.match(TokenKind.EQ):
            value = self._parse_assignment()
            if not self._is_lvalue(expr):
                raise ParseError("Invalid assignment target")
            return Assign(expr, value)
        return expr

    def _parse_logic_or(self) -> Expr:
        expr = self._parse_logic_and()
        while self.ts.match(TokenKind.OROR):
            right = self._parse_logic_and()
            expr = Binary("||", expr, right)
        return expr

    def _parse_logic_and(self) -> Expr:
        expr = self._parse_equality()
        while self.ts.match(TokenKind.ANDAND):
            right = self._parse_equality()
            expr = Binary("&&", expr, right)
        return expr

    def _parse_equality(self) -> Expr:
        expr = self._parse_relational()
        while True:
            if self.ts.match(TokenKind.EQEQ):
                right = self._parse_relational()
                expr = Binary("==", expr, right)
            elif self.ts.match(TokenKind.BANGEQ):
                right = self._parse_relational()
                expr = Binary("!=", expr, right)
            else:
                return expr

    def _parse_relational(self) -> Expr:
        expr = self._parse_additive()
        while True:
            if self.ts.match(TokenKind.LT):
                right = self._parse_additive()
                expr = Binary("<", expr, right)
            elif self.ts.match(TokenKind.LTE):
                right = self._parse_additive()
                expr = Binary("<=", expr, right)
            elif self.ts.match(TokenKind.GT):
                right = self._parse_additive()
                expr = Binary(">", expr, right)
            elif self.ts.match(TokenKind.GTE):
                right = self._parse_additive()
                expr = Binary(">=", expr, right)
            else:
                return expr

    def _parse_additive(self) -> Expr:
        expr = self._parse_multiplicative()
        while True:
            if self.ts.match(TokenKind.PLUS):
                right = self._parse_multiplicative()
                expr = Binary("+", expr, right)
            elif self.ts.match(TokenKind.MINUS):
                right = self._parse_multiplicative()
                expr = Binary("-", expr, right)
            else:
                return expr

    def _parse_multiplicative(self) -> Expr:
        expr = self._parse_unary()
        while True:
            if self.ts.match(TokenKind.STAR):
                right = self._parse_unary()
                expr = Binary("*", expr, right)
            elif self.ts.match(TokenKind.SLASH):
                right = self._parse_unary()
                expr = Binary("/", expr, right)
            elif self.ts.match(TokenKind.PERCENT):
                right = self._parse_unary()
                expr = Binary("%", expr, right)
            else:
                return expr

    def _parse_unary(self) -> Expr:
        if self.ts.match(TokenKind.MINUS):
            return Unary("-", self._parse_unary())
        if self.ts.match(TokenKind.BANG):
            return Unary("!", self._parse_unary())
        if self.ts.match(TokenKind.STAR):
            return Unary("*", self._parse_unary())
        if self.ts.match(TokenKind.AMP):
            return Unary("&", self._parse_unary())
        return self._parse_postfix()

    def _parse_postfix(self) -> Expr:
        expr = self._parse_primary()
        while True:
            if self.ts.match(TokenKind.LPAREN):
                args: List[Expr] = []
                if self.ts.peek().kind != TokenKind.RPAREN:
                    while True:
                        args.append(self._parse_expression())
                        if not self.ts.match(TokenKind.COMMA):
                            break
                self.ts.consume(TokenKind.RPAREN, "Expected ')' after arguments")
                expr = Call(expr, args)
                continue
            if self.ts.match(TokenKind.DOT):
                name = self._consume_ident("Expected field name after '.'")
                expr = Field(expr, name)
                continue
            break
        return expr

    def _parse_primary(self) -> Expr:
        tok = self.ts.peek()
        if self.ts.match(TokenKind.INT):
            return IntLit(int(tok.lexeme))
        if self.ts.match(TokenKind.KW_TRUE):
            return BoolLit(True)
        if self.ts.match(TokenKind.KW_FALSE):
            return BoolLit(False)
        if self.ts.match(TokenKind.KW_NULL):
            return NullLit()
        if self.ts.match(TokenKind.IDENT):
            return Var(tok.lexeme)
        if self.ts.match(TokenKind.LPAREN):
            expr = self._parse_expression()
            self.ts.consume(TokenKind.RPAREN, "Expected ')' after expression")
            return expr
        raise ParseError(f"Unexpected token {tok.kind} at {tok.line}:{tok.col}")

    def _parse_defer_call(self) -> Call:
        expr = self._parse_expression()
        if not isinstance(expr, Call):
            raise ParseError("defer requires a call expression")
        return expr

    def _is_lvalue(self, expr: Expr) -> bool:
        return isinstance(expr, (Var, Field)) or (
            isinstance(expr, Unary) and expr.op == "*"
        )

    def _consume_ident(self, message: str) -> str:
        tok = self.ts.peek()
        if tok.kind != TokenKind.IDENT:
            raise ParseError(f"{message} at {tok.line}:{tok.col}")
        self.ts.advance()
        return tok.lexeme
