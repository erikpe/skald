#include "skald_runtime.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

static int fail_next_allocation;

void* __real_malloc(size_t size);

void* __wrap_malloc(size_t size) {
    if (fail_next_allocation != 0) {
        fail_next_allocation = 0;
        return NULL;
    }
    return __real_malloc(size);
}

static void require_cleared_trace(void) {
    if (ska_rt_trace_top != NULL) {
        _Exit(97);
    }
}

__attribute__((constructor)) static void install_trace_exit_check(void) {
    if (atexit(require_cleared_trace) != 0) {
        _Exit(98);
    }
}

int64_t ska_test_trace_depth(void) {
    int64_t depth = 0;
    for (const SkaRtTraceFrame* frame = ska_rt_trace_top; frame != NULL;
         frame = frame->previous) {
        ++depth;
    }
    return depth;
}

void ska_test_external_panic(void) {
    static const uint8_t message[] = "external failure";
    ska_rt_panic(message, sizeof(message) - 1u);
}

void ska_test_fail_next_allocation(void) {
    fail_next_allocation = 1;
}
