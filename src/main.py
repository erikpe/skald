from __future__ import annotations

import argparse
from pathlib import Path
from pprint import pprint

from codegen import Codegen
from lexer import Lexer
from lower import lower_program
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
    parser.add_argument(
        "--lower",
        action="store_true",
        help="Lower returns to single-exit form (dump lowered AST)",
    )
    parser.add_argument(
        "--emit",
        nargs="?",
        const="-",
        help="Emit x86-64 assembly (optional path, default stdout)",
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
    elif args.lower:
        symbols = build_global_symbols(program)
        lowered = lower_program(program, symbols)
        pprint(lowered)
    elif args.emit is not None:
        symbols = build_global_symbols(program)
        typecheck_program(program, symbols)
        lowered = lower_program(program, symbols)
        asm = Codegen(symbols).emit_program(lowered)
        if args.emit == "-":
            print(asm)
        else:
            Path(args.emit).write_text(asm, encoding="utf-8")
    else:
        pprint(program)


if __name__ == "__main__":
    main()
