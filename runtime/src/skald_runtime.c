#define _POSIX_C_SOURCE 200809L

#include "skald_runtime.h"

#include <errno.h>
#include <limits.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdlib.h>
#include <unistd.h>

_Static_assert(CHAR_BIT == 8, "Skald runtime requires eight-bit bytes");

static _Noreturn void ska_rt_terminate_unsuccessfully(void) {
    /* Runtime boundary failures are unrecoverable. _Exit also avoids an
       implicit attempt to flush stdout after an output failure. */
    _Exit(EXIT_FAILURE);
}

static _Noreturn void ska_rt_runtime_defect(void) {
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

_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length) {
    static const uint8_t prefix[] = "panic: ";
    static const uint8_t line_feed[] = "\n";

    if (bytes == NULL && length != UINT64_C(0)) {
        ska_rt_runtime_defect();
    }
    if (!ska_rt_write_stderr_bytes(prefix, (uint64_t)(sizeof(prefix) - 1))
        || !ska_rt_write_stderr_bytes(bytes, length)
        || !ska_rt_write_stderr_bytes(line_feed, (uint64_t)(sizeof(line_feed) - 1))) {
        ska_rt_terminate_unsuccessfully();
    }
    ska_rt_terminate_unsuccessfully();
}
