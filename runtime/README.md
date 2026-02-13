Runtime C helpers for the stage-0 compiler.

Exports:
- print_i64(i64)
- print_u64(u64)
- print_u8(u8)
- read_i64() -> i64
- malloc_u64(u64) -> *u8
- free_ptr(*u8)
- realloc_ptr(*u8, u64) -> *u8
