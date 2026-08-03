#include "skald_runtime.h"

#include <inttypes.h>
#include <limits.h>
#include <stdio.h>

_Static_assert(CHAR_BIT == 8, "runtime test requires eight-bit bytes");
_Static_assert(SKALD_RUNTIME_ABI_VERSION == UINT64_C(8),
               "runtime contract requires ABI version 8");

int main(void) {
    SKALD_RUNTIME_ABI_MARKER();
    const uint64_t reported_version = ska_rt_abi_version();

    if (reported_version != SKALD_RUNTIME_ABI_VERSION) {
        fprintf(stderr,
                "runtime ABI version mismatch: header=%" PRIu64 ", runtime=%" PRIu64 "\n",
                SKALD_RUNTIME_ABI_VERSION,
                reported_version);
        return 1;
    }

    return 0;
}
