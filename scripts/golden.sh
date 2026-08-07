#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repository=$(cd -- "$script_directory/.." && pwd)

make -C "$repository" golden-tools
cd -- "$repository"
exec "$repository/target/debug/skald-golden" \
    --compiler "$repository/target/debug/skac" \
    "$@"
