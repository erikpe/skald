#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <execinfo.h>
#include <unistd.h>

void print_i64(int64_t x) {
    printf("%lld\n", (long long)x);
}

void print_u64(uint64_t x) {
    printf("%llu\n", (unsigned long long)x);
}

void print_u8(uint8_t x) {
    printf("%u\n", (unsigned int)x);
}

void print_bool(uint8_t x) {
    if (x != 0) {
        printf("true\n");
    } else {
        printf("false\n");
    }
}

int64_t read_i64(void) {
    long long x = 0;
    if (scanf("%lld", &x) != 1) {
        return 0;
    }
    return (int64_t)x;
}

void *malloc_u64(uint64_t size) {
    return malloc((size_t)size);
}

void free_ptr(void *p) {
    free(p);
}

void *realloc_ptr(void *p, uint64_t size) {
    return realloc(p, (size_t)size);
}

static void emit_stack_trace(void) {
    void *frames[64];
    int count = backtrace(frames, 64);
    fprintf(stderr, "stack trace (%d frames):\n", count);
    backtrace_symbols_fd(frames, count, STDERR_FILENO);
}

static void panic_common(const char *message) {
    fflush(stdout);
    fprintf(stderr, "PANIC: %s\n", message);
    emit_stack_trace();
    abort();
}

void panic(void) {
    panic_common("explicit panic");
}

void panic_vec_i64_null(void) {
    panic_common("VecI64 operation on null pointer");
}

void panic_vec_i64_oob(uint64_t idx, uint64_t len) {
    fflush(stdout);
    fprintf(stderr, "PANIC: VecI64 index out of bounds (idx=%llu, len=%llu)\n",
            (unsigned long long)idx,
            (unsigned long long)len);
    emit_stack_trace();
    abort();
}

void panic_vec_i64_empty_pop(void) {
    panic_common("VecI64 pop on empty vector");
}

void panic_vec_i64_oom(uint64_t requested_cap) {
    fflush(stdout);
    fprintf(stderr,
            "PANIC: VecI64 allocation failed while growing (requested_cap=%llu)\n",
            (unsigned long long)requested_cap);
    emit_stack_trace();
    abort();
}
