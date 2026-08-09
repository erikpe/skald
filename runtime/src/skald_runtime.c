#include "skald_runtime.h"

#include <limits.h>
#include <stddef.h>
#include <stdlib.h>

_Static_assert(CHAR_BIT == 8, "Skald runtime requires eight-bit bytes");

static _Noreturn void ska_rt_runtime_defect(void) {
    abort();
}

void SKALD_RUNTIME_ABI_MARKER(void) {
}

uint64_t ska_rt_abi_version(void) {
    return SKALD_RUNTIME_ABI_VERSION;
}

void* ska_rt_alloc(uint64_t byte_count) {
    static const uint8_t allocation_failure[] = "memory allocation failed";
    const size_t allocation_size = (size_t)byte_count;
    void* allocation;

    if (byte_count == UINT64_C(0) || (uint64_t)allocation_size != byte_count) {
        ska_rt_runtime_defect();
    }
    allocation = malloc(allocation_size);
    if (allocation == NULL) {
        ska_rt_panic(allocation_failure, (uint64_t)(sizeof(allocation_failure) - 1));
    }
    return allocation;
}

void ska_rt_free(void* allocation) {
    free(allocation);
}
