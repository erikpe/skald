set -eu

nm_command=$1
archive=$2

actual=$(
    "$nm_command" -g --defined-only "$archive" |
        awk 'NF >= 3 { print $NF }' |
        LC_ALL=C sort
)
expected='ska_rt_abi_v9
ska_rt_abi_version
ska_rt_alloc
ska_rt_free
ska_rt_io_close
ska_rt_io_open
ska_rt_io_read
ska_rt_io_standard_handle
ska_rt_io_write
ska_rt_panic
ska_rt_trace_top'

if [ "$actual" != "$expected" ]; then
    printf '%s\n' 'runtime archive exports do not match the version-9 ABI' >&2
    printf '%s\n' 'expected:' "$expected" 'actual:' "$actual" >&2
    exit 1
fi
