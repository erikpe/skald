from __future__ import annotations

import argparse
from pathlib import Path
from pprint import pprint

from lexer import Lexer
from parser import Parser
from symbols import build_global_symbols
from typecheck import typecheck_program


def main() -> None:
    parser = argparse.ArgumentParser(description="Lex and parse a source file")
    parser.add_argument("path", help="Path to source file")
    parser.add_argument(
        "--tokens",
        action="store_true",
        help="Print tokens before parsing",
    )
    parser.add_argument(
        "--symbols",
        action="store_true",
        help="Print global symbols and struct layouts",
    )
    parser.add_argument(
        "--typecheck",
        action="store_true",
        help="Typecheck the parsed program",
    )
    args = parser.parse_args()

    source = Path(args.path).read_text(encoding="utf-8")
    tokens = Lexer(source).tokenize()
    if args.tokens:
        for tok in tokens:
            print(f"{tok.line}:{tok.col} {tok.kind} {tok.lexeme}")
    program = Parser(tokens).parse_program()
    if args.symbols:
        symbols = build_global_symbols(program)
        pprint(symbols)
    elif args.typecheck:
        symbols = build_global_symbols(program)
        typecheck_program(program, symbols)
        print("Typecheck OK")
    else:
        pprint(program)


if __name__ == "__main__":
    main()
