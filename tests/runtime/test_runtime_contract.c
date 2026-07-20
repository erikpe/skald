#include "skald_runtime.h"

#include <float.h>
#include <inttypes.h>
#include <limits.h>
#include <stdio.h>

#if !defined(__STDC_IEC_559__) || __STDC_IEC_559__ != 1
#error "runtime test requires IEC 60559 / IEEE-754 floating-point semantics"
#endif

_Static_assert(CHAR_BIT == 8, "runtime test requires eight-bit bytes");
_Static_assert(SKALD_RUNTIME_ABI_VERSION == UINT64_C(4),
               "runtime contract requires ABI version 4");
_Static_assert(sizeof(double) == sizeof(uint64_t), "runtime test requires a 64-bit double");
_Static_assert(FLT_RADIX == 2, "runtime test requires a binary double");
_Static_assert(DBL_MANT_DIG == 53, "runtime test requires an IEEE-754 binary64 significand");
_Static_assert(DBL_MIN_EXP == -1021, "runtime test requires the binary64 exponent range");
_Static_assert(DBL_MAX_EXP == 1024, "runtime test requires the binary64 exponent range");

int main(void) {
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
