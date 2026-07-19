#include "skald_runtime.h"

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

enum {
    SKA_RT_I64_DECIMAL_DIGITS = 19,
    SKA_RT_I64_LINE_CAPACITY = SKA_RT_I64_DECIMAL_DIGITS + 2
};

static size_t ska_rt_format_i64_line(char output[SKA_RT_I64_LINE_CAPACITY], int64_t value) {
    char reversed_digits[SKA_RT_I64_DECIMAL_DIGITS];
    size_t digit_count = 0;
    size_t output_length = 0;
    uint64_t magnitude;

    if (value < 0) {
        /* Unsigned arithmetic avoids overflowing when value is INT64_MIN. */
        magnitude = UINT64_C(0) - (uint64_t)value;
        output[output_length++] = '-';
    } else {
        magnitude = (uint64_t)value;
    }

    do {
        reversed_digits[digit_count++] = (char)('0' + (magnitude % UINT64_C(10)));
        magnitude /= UINT64_C(10);
    } while (magnitude != UINT64_C(0));

    while (digit_count != 0) {
        output[output_length++] = reversed_digits[--digit_count];
    }
    output[output_length++] = '\n';
    return output_length;
}

static void ska_rt_output_failure(void) {
    /* Output failure is unrecoverable at the bootstrap ABI. _Exit avoids a
       second implicit attempt to flush the already-failed stdout stream. */
    _Exit(EXIT_FAILURE);
}

uint64_t ska_rt_abi_version(void) {
    return SKALD_RUNTIME_ABI_VERSION;
}

void ska_rt_println_i64(int64_t value) {
    char output[SKA_RT_I64_LINE_CAPACITY];
    const size_t output_length = ska_rt_format_i64_line(output, value);

    if (fwrite(output, sizeof(output[0]), output_length, stdout) != output_length ||
        fflush(stdout) == EOF) {
        ska_rt_output_failure();
    }
}
