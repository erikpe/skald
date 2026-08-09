#include "skald_runtime.h"

#include <inttypes.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>

_Static_assert(CHAR_BIT == 8, "runtime test requires eight-bit bytes");
_Static_assert(SKALD_RUNTIME_ABI_VERSION == UINT64_C(9),
               "runtime contract requires ABI version 9");
_Static_assert(sizeof(SkaRtTraceContext) == 32, "unexpected trace-context size");
_Static_assert(offsetof(SkaRtTraceContext, name) == 0, "unexpected trace name offset");
_Static_assert(offsetof(SkaRtTraceContext, name_length) == 8,
               "unexpected trace name-length offset");
_Static_assert(offsetof(SkaRtTraceContext, path) == 16, "unexpected trace path offset");
_Static_assert(offsetof(SkaRtTraceContext, path_length) == 24,
               "unexpected trace path-length offset");
_Static_assert(sizeof(SkaRtTraceLocation) == 24, "unexpected trace-location size");
_Static_assert(offsetof(SkaRtTraceLocation, context) == 0,
               "unexpected trace context offset");
_Static_assert(offsetof(SkaRtTraceLocation, line) == 8, "unexpected trace line offset");
_Static_assert(offsetof(SkaRtTraceLocation, column) == 16,
               "unexpected trace column offset");
_Static_assert(sizeof(SkaRtTraceFrame) == 16, "unexpected trace-frame size");
_Static_assert(offsetof(SkaRtTraceFrame, previous) == 0,
               "unexpected previous-frame offset");
_Static_assert(offsetof(SkaRtTraceFrame, location) == 8,
               "unexpected frame-location offset");

int main(void) {
    SKALD_RUNTIME_ABI_MARKER();
    const uint64_t reported_version = ska_rt_abi_version();

    if (ska_rt_trace_top != NULL) {
        fprintf(stderr, "runtime trace top is not initially null\n");
        return 1;
    }

    if (reported_version != SKALD_RUNTIME_ABI_VERSION) {
        fprintf(stderr,
                "runtime ABI version mismatch: header=%" PRIu64 ", runtime=%" PRIu64 "\n",
                SKALD_RUNTIME_ABI_VERSION,
                reported_version);
        return 1;
    }

    return 0;
}
