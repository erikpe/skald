#define _POSIX_C_SOURCE 200809L

#include "skald_runtime.h"

#include <errno.h>
#include <limits.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdlib.h>
#include <unistd.h>

enum {
    SKA_RT_TRACE_LIMIT = 256,
    SKA_RT_U64_DECIMAL_CAPACITY = 20
};

static const uint8_t PANIC_PREFIX[] = "panic: ";
static const uint8_t STACKTRACE_HEADER[] = "stacktrace:\n";
static const uint8_t STACKTRACE_FRAME_PREFIX[] = "  at ";
static const uint8_t STACKTRACE_LOCATION_PREFIX[] = " (";
static const uint8_t STACKTRACE_POSITION_SEPARATOR[] = ":";
static const uint8_t STACKTRACE_FRAME_SUFFIX[] = ")\n";
static const uint8_t STACKTRACE_OMITTED[] = "  ... outer frames omitted\n";
static const uint8_t LINE_FEED[] = "\n";

#if defined(__GNUC__) || defined(__clang__)
__attribute__((visibility("hidden")))
#endif
_Thread_local SkaRtTraceFrame* ska_rt_trace_top;

_Static_assert(CHAR_BIT == 8, "Skald runtime requires eight-bit bytes");
_Static_assert(sizeof(SkaRtTraceContext) == 32,
               "Skald runtime trace context must occupy 32 bytes");
_Static_assert(sizeof(SkaRtTraceLocation) == 24,
               "Skald runtime trace location must occupy 24 bytes");
_Static_assert(sizeof(SkaRtTraceFrame) == 16,
               "Skald runtime trace frame must occupy 16 bytes");

static _Noreturn void ska_rt_terminate_unsuccessfully(void) {
    /* Runtime boundary failures are unrecoverable. _Exit also avoids an
       implicit attempt to flush stdout after an output failure. */
    _Exit(EXIT_FAILURE);
}

static _Noreturn void ska_rt_panic_contract_defect(void) {
    abort();
}

static bool ska_rt_write_stderr_bytes(const uint8_t* bytes, uint64_t length) {
    while (length != UINT64_C(0)) {
        const size_t chunk_length =
            length > (uint64_t)SSIZE_MAX ? (size_t)SSIZE_MAX : (size_t)length;
        const ssize_t written = write(STDERR_FILENO, bytes, chunk_length);

        if (written < 0) {
            if (errno == EINTR) {
                continue;
            }
            return false;
        }
        if (written == 0) {
            return false;
        }
        bytes += (size_t)written;
        length -= (uint64_t)written;
    }
    return true;
}

static bool ska_rt_write_literal(const uint8_t* bytes, size_t length) {
    return ska_rt_write_stderr_bytes(bytes, (uint64_t)length);
}

static bool ska_rt_write_u64_decimal(uint64_t value) {
    uint8_t digits[SKA_RT_U64_DECIMAL_CAPACITY];
    size_t first = sizeof(digits);

    do {
        digits[--first] = (uint8_t)('0' + (value % UINT64_C(10)));
        value /= UINT64_C(10);
    } while (value != UINT64_C(0));

    return ska_rt_write_stderr_bytes(digits + first, (uint64_t)(sizeof(digits) - first));
}

static bool ska_rt_write_trace_frame(const SkaRtTraceFrame* frame) {
    const SkaRtTraceLocation* location = frame->location;
    const SkaRtTraceContext* context = location->context;

    return ska_rt_write_literal(
               STACKTRACE_FRAME_PREFIX, sizeof(STACKTRACE_FRAME_PREFIX) - 1)
        && ska_rt_write_stderr_bytes(context->name, context->name_length)
        && ska_rt_write_literal(
            STACKTRACE_LOCATION_PREFIX, sizeof(STACKTRACE_LOCATION_PREFIX) - 1)
        && ska_rt_write_stderr_bytes(context->path, context->path_length)
        && ska_rt_write_literal(
            STACKTRACE_POSITION_SEPARATOR, sizeof(STACKTRACE_POSITION_SEPARATOR) - 1)
        && ska_rt_write_u64_decimal(location->line)
        && ska_rt_write_literal(
            STACKTRACE_POSITION_SEPARATOR, sizeof(STACKTRACE_POSITION_SEPARATOR) - 1)
        && ska_rt_write_u64_decimal(location->column)
        && ska_rt_write_literal(
            STACKTRACE_FRAME_SUFFIX, sizeof(STACKTRACE_FRAME_SUFFIX) - 1);
}

static bool ska_rt_write_stacktrace(void) {
    const SkaRtTraceFrame* frame = ska_rt_trace_top;
    size_t rendered = 0;

    if (frame == NULL) {
        return true;
    }
    if (!ska_rt_write_literal(STACKTRACE_HEADER, sizeof(STACKTRACE_HEADER) - 1)) {
        return false;
    }
    while (frame != NULL && rendered < SKA_RT_TRACE_LIMIT) {
        if (!ska_rt_write_trace_frame(frame)) {
            return false;
        }
        frame = frame->previous;
        ++rendered;
    }
    return frame == NULL
        || ska_rt_write_literal(STACKTRACE_OMITTED, sizeof(STACKTRACE_OMITTED) - 1);
}

_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length) {
    if (bytes == NULL && length != UINT64_C(0)) {
        ska_rt_panic_contract_defect();
    }
    if (!ska_rt_write_literal(PANIC_PREFIX, sizeof(PANIC_PREFIX) - 1)
        || !ska_rt_write_stderr_bytes(bytes, length)
        || !ska_rt_write_literal(LINE_FEED, sizeof(LINE_FEED) - 1)
        || !ska_rt_write_stacktrace()) {
        ska_rt_terminate_unsuccessfully();
    }
    ska_rt_terminate_unsuccessfully();
}
