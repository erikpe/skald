#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

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
