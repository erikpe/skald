from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Dict


class TokenKind(str, Enum):
    IDENT = "IDENT"
    INT = "INT"
    EOF = "EOF"

    KW_FN = "KW_FN"
    KW_STRUCT = "KW_STRUCT"
    KW_VAR = "KW_VAR"
    KW_IF = "KW_IF"
    KW_ELSE = "KW_ELSE"
    KW_WHILE = "KW_WHILE"
    KW_RETURN = "KW_RETURN"
    KW_DEFER = "KW_DEFER"
    KW_TRUE = "KW_TRUE"
    KW_FALSE = "KW_FALSE"
    KW_NULL = "KW_NULL"
    KW_EXTERN = "KW_EXTERN"

    PLUS = "PLUS"
    MINUS = "MINUS"
    STAR = "STAR"
    SLASH = "SLASH"
    PERCENT = "PERCENT"

    EQ = "EQ"
    EQEQ = "EQEQ"
    BANGEQ = "BANGEQ"
    LT = "LT"
    LTE = "LTE"
    GT = "GT"
    GTE = "GTE"

    ANDAND = "ANDAND"
    OROR = "OROR"
    AMP = "AMP"
    BANG = "BANG"

    LPAREN = "LPAREN"
    RPAREN = "RPAREN"
    LBRACE = "LBRACE"
    RBRACE = "RBRACE"
    LBRACKET = "LBRACKET"
    RBRACKET = "RBRACKET"

    SEMI = "SEMI"
    COMMA = "COMMA"
    COLON = "COLON"
    DOT = "DOT"
    ARROW = "ARROW"


KEYWORDS: Dict[str, TokenKind] = {
    "fn": TokenKind.KW_FN,
    "struct": TokenKind.KW_STRUCT,
    "var": TokenKind.KW_VAR,
    "if": TokenKind.KW_IF,
    "else": TokenKind.KW_ELSE,
    "while": TokenKind.KW_WHILE,
    "return": TokenKind.KW_RETURN,
    "defer": TokenKind.KW_DEFER,
    "true": TokenKind.KW_TRUE,
    "false": TokenKind.KW_FALSE,
    "null": TokenKind.KW_NULL,
    "extern": TokenKind.KW_EXTERN,
}


@dataclass(frozen=True)
class Token:
    kind: TokenKind
    lexeme: str
    filepath: str
    line: int
    col: int
