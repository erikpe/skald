#include "skald_runtime.h"

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int verify_allocation(uint64_t byte_count, unsigned char fill) {
    unsigned char* allocation = ska_rt_alloc(byte_count);
    size_t index;

    if ((uintptr_t)allocation % _Alignof(max_align_t) != 0) {
        fprintf(stderr, "runtime allocation is not suitably aligned\n");
        ska_rt_free(allocation);
        return 1;
    }
    memset(allocation, fill, (size_t)byte_count);
    for (index = 0; index < (size_t)byte_count; ++index) {
        if (allocation[index] != fill) {
            fprintf(stderr, "runtime allocation is not writable at byte %zu\n", index);
            ska_rt_free(allocation);
            return 1;
        }
    }
    ska_rt_free(allocation);
    return 0;
}

int main(void) {
    if (verify_allocation(UINT64_C(1), UINT8_C(0xa5)) != 0) {
        return 1;
    }
    if (verify_allocation(UINT64_C(16), UINT8_C(0x5a)) != 0) {
        return 1;
    }
    if (verify_allocation(UINT64_C(257), UINT8_C(0xc3)) != 0) {
        return 1;
    }
    if (verify_allocation(UINT64_C(4096), UINT8_C(0x3c)) != 0) {
        return 1;
    }
    return 0;
}
