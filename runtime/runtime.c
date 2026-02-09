#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

void print_i64(int64_t x) {
    printf("%lld\n", (long long)x);
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
