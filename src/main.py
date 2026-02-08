from __future__ import annotations

import argparse
from pathlib import Path

from lexer import Lexer


def main() -> None:
    parser = argparse.ArgumentParser(description="Lex a source file and print tokens")
    parser.add_argument("path", help="Path to source file")
    args = parser.parse_args()

    source = Path(args.path).read_text(encoding="utf-8")
    tokens = Lexer(source).tokenize()
    for tok in tokens:
        print(f"{tok.line}:{tok.col} {tok.kind} {tok.lexeme}")


if __name__ == "__main__":
    main()
