from __future__ import annotations

from typing import List

from tokens import KEYWORDS, Token, TokenKind


class Lexer:
    def __init__(self, source: str) -> None:
        self.source = source
        self.length = len(source)
        self.index = 0
        self.line = 1
        self.col = 1

    def tokenize(self) -> List[Token]:
        tokens: List[Token] = []
        while True:
            self._skip_whitespace_and_comments()
            if self._is_at_end():
                tokens.append(Token(TokenKind.EOF, "", self.line, self.col))
                return tokens

            start_line = self.line
            start_col = self.col
            ch = self._advance()

            if ch.isalpha() or ch == "_":
                ident = ch + self._consume_while(self._is_ident_tail)
                kind = KEYWORDS.get(ident, TokenKind.IDENT)
                tokens.append(Token(kind, ident, start_line, start_col))
                continue

            if ch.isdigit():
                num = ch + self._consume_while(str.isdigit)
                tokens.append(Token(TokenKind.INT, num, start_line, start_col))
                continue

            matched = self._match_operator_or_punct(ch)
            if matched is not None:
                kind, lexeme = matched
                tokens.append(Token(kind, lexeme, start_line, start_col))
                continue

            raise ValueError(f"Unexpected character '{ch}' at {start_line}:{start_col}")

    def _match_operator_or_punct(self, ch: str) -> tuple[TokenKind, str] | None:
        if ch == "=":
            if self._match("="):
                return TokenKind.EQEQ, "=="
            return TokenKind.EQ, "="
        if ch == "!":
            if self._match("="):
                return TokenKind.BANGEQ, "!="
            return TokenKind.BANG, "!"
        if ch == "<":
            if self._match("="):
                return TokenKind.LTE, "<="
            return TokenKind.LT, "<"
        if ch == ">":
            if self._match("="):
                return TokenKind.GTE, ">="
            return TokenKind.GT, ">"
        if ch == "&":
            if self._match("&"):
                return TokenKind.ANDAND, "&&"
            return TokenKind.AMP, "&"
        if ch == "|":
            if self._match("|"):
                return TokenKind.OROR, "||"
            raise ValueError(f"Unexpected character '|' at {self.line}:{self.col - 1}")
        if ch == "-" and self._match(">"):
            return TokenKind.ARROW, "->"

        single = {
            "+": TokenKind.PLUS,
            "-": TokenKind.MINUS,
            "*": TokenKind.STAR,
            "/": TokenKind.SLASH,
            "%": TokenKind.PERCENT,
            "(": TokenKind.LPAREN,
            ")": TokenKind.RPAREN,
            "{": TokenKind.LBRACE,
            "}": TokenKind.RBRACE,
            "[": TokenKind.LBRACKET,
            "]": TokenKind.RBRACKET,
            ";": TokenKind.SEMI,
            ",": TokenKind.COMMA,
            ":": TokenKind.COLON,
            ".": TokenKind.DOT,
        }
        kind = single.get(ch)
        if kind is None:
            return None
        return kind, ch

    def _skip_whitespace_and_comments(self) -> None:
        while True:
            if self._is_at_end():
                return
            ch = self._peek()
            if ch in " \t\r\n":
                self._advance()
                continue
            if ch == "/" and self._peek_next() == "/":
                self._advance()
                self._advance()
                while not self._is_at_end() and self._peek() != "\n":
                    self._advance()
                continue
            if ch == "/" and self._peek_next() == "*":
                self._advance()
                self._advance()
                self._consume_block_comment()
                continue
            return

    def _consume_block_comment(self) -> None:
        while not self._is_at_end():
            if self._peek() == "*" and self._peek_next() == "/":
                self._advance()
                self._advance()
                return
            self._advance()
        raise ValueError(f"Unterminated block comment at {self.line}:{self.col}")

    def _consume_while(self, predicate) -> str:
        out = []
        while not self._is_at_end() and predicate(self._peek()):
            out.append(self._advance())
        return "".join(out)

    def _is_ident_tail(self, ch: str) -> bool:
        return ch.isalnum() or ch == "_"

    def _is_at_end(self) -> bool:
        return self.index >= self.length

    def _peek(self) -> str:
        if self._is_at_end():
            return "\0"
        return self.source[self.index]

    def _peek_next(self) -> str:
        if self.index + 1 >= self.length:
            return "\0"
        return self.source[self.index + 1]

    def _advance(self) -> str:
        ch = self.source[self.index]
        self.index += 1
        if ch == "\n":
            self.line += 1
            self.col = 1
        else:
            self.col += 1
        return ch

    def _match(self, expected: str) -> bool:
        if self._is_at_end() or self.source[self.index] != expected:
            return False
        self._advance()
        return True
