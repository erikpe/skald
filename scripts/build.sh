#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <source.ska> [output]"
  exit 1
fi

SRC="$1"
OUT="${2:-a.out}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
ASM="$BUILD_DIR/out.s"

mkdir -p "$BUILD_DIR"

python3 "$ROOT_DIR/src/main.py" "$SRC" --emit "$ASM"
gcc "$ASM" "$ROOT_DIR/runtime/runtime.c" -o "$OUT"

echo "Built: $OUT"
