#!/bin/sh
set -eu

repository=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
temporary="${TMPDIR:-/tmp}/skald-runtime-build-configuration-$$"
build_directory="$temporary/build"
log="$temporary/invocations.log"
cc_one="$temporary/cc-one"
cc_two="$temporary/cc-two"
archiver="$temporary/ar"

cleanup() {
    rm -rf "$temporary"
}
trap cleanup 0 1 2 3 15
mkdir -p "$temporary"
: > "$log"

cat > "$cc_one" <<'EOF'
#!/bin/sh
printf '%s\n' cc >> "$SKALD_RUNTIME_BUILD_TEST_LOG"
exec cc "$@"
EOF
cp "$cc_one" "$cc_two"
cat > "$archiver" <<'EOF'
#!/bin/sh
printf '%s\n' ar >> "$SKALD_RUNTIME_BUILD_TEST_LOG"
exec ar "$@"
EOF
chmod +x "$cc_one" "$cc_two" "$archiver"

run_build() {
    compiler=$1
    flags=$2
    SKALD_RUNTIME_BUILD_TEST_LOG="$log" \
        make -s --no-print-directory -C "$repository/runtime" \
        BUILD_DIR="$build_directory" \
        CC="$compiler" \
        AR="$archiver" \
        CFLAGS="$flags" \
        all
}

assert_invocations() {
    expected_cc=$1
    expected_ar=$2
    actual_cc=$(awk '$0 == "cc" { count += 1 } END { print count + 0 }' "$log")
    actual_ar=$(awk '$0 == "ar" { count += 1 } END { print count + 0 }' "$log")
    if test "$actual_cc" -ne "$expected_cc" || test "$actual_ar" -ne "$expected_ar"; then
        printf 'expected %s compiler and %s archiver invocations, got %s and %s\n' \
            "$expected_cc" "$expected_ar" "$actual_cc" "$actual_ar" >&2
        exit 1
    fi
}

initial_flags='-O0'
changed_flags='-O1 -DSKALD_RUNTIME_BUILD_CONFIGURATION_TEST=1'

run_build "$cc_one" "$initial_flags"
assert_invocations 3 1
test -f "$build_directory/libskald_runtime.a"
grep -Fqx 'format=1' "$build_directory/build-config.txt"
grep -Fqx "cc=$cc_one" "$build_directory/build-config.txt"
grep -Fqx "ar=$archiver" "$build_directory/build-config.txt"
grep -Fqx "cflags=$initial_flags -std=c11 -Wall -Wextra -Werror -Iinclude" \
    "$build_directory/build-config.txt"

run_build "$cc_one" "$initial_flags"
assert_invocations 3 1

run_build "$cc_one" "$changed_flags"
assert_invocations 6 2
grep -Fqx "cflags=$changed_flags -std=c11 -Wall -Wextra -Werror -Iinclude" \
    "$build_directory/build-config.txt"

run_build "$cc_two" "$changed_flags"
assert_invocations 9 3
grep -Fqx "cc=$cc_two" "$build_directory/build-config.txt"
