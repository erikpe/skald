Runtime C helpers for the stage-0 compiler.

Exports:
- print_i64(i64)
- print_u64(u64)
- print_u8(u8)
- print_bool(bool)
- read_i64() -> i64
- malloc_u64(u64) -> *u8
- free_ptr(*u8)
- realloc_ptr(*u8, u64) -> *u8
- panic() -> unit (prints stack trace, aborts)
- panic_vec_i64_null() -> unit (prints stack trace, aborts)
- panic_vec_i64_oob(u64 idx, u64 len) -> unit (prints stack trace, aborts)
- panic_vec_i64_empty_pop() -> unit (prints stack trace, aborts)
- panic_vec_i64_oom(u64 requested_cap) -> unit (prints stack trace, aborts)
