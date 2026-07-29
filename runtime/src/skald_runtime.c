#define _POSIX_C_SOURCE 200809L

#include "skald_runtime.h"

#include <errno.h>
#include <float.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

enum {
    SKA_RT_I64_DECIMAL_DIGITS = 19,
    SKA_RT_U64_DECIMAL_DIGITS = 20,
    SKA_RT_UNSIGNED_LINE_CAPACITY = SKA_RT_U64_DECIMAL_DIGITS + 1,
    SKA_RT_I64_LINE_CAPACITY = SKA_RT_I64_DECIMAL_DIGITS + 2,
    SKA_RT_F64_HEX_DIGITS = 16,
    SKA_RT_F64_BITS_LINE_CAPACITY = 2 + SKA_RT_F64_HEX_DIGITS + 1
};

#if !defined(__STDC_IEC_559__) || __STDC_IEC_559__ != 1
#error "Skald f64 requires IEC 60559 / IEEE-754 floating-point semantics"
#endif

_Static_assert(CHAR_BIT == 8, "Skald runtime requires eight-bit bytes");
_Static_assert(sizeof(double) == sizeof(uint64_t), "Skald f64 requires a 64-bit C double");
_Static_assert(FLT_RADIX == 2, "Skald f64 requires a binary C double");
_Static_assert(DBL_MANT_DIG == 53, "Skald f64 requires an IEEE-754 binary64 significand");
_Static_assert(DBL_MIN_EXP == -1021, "Skald f64 requires the IEEE-754 binary64 exponent range");
_Static_assert(DBL_MAX_EXP == 1024, "Skald f64 requires the IEEE-754 binary64 exponent range");

static size_t ska_rt_append_u64_decimal(char* output, size_t output_length, uint64_t value) {
    char reversed_digits[SKA_RT_U64_DECIMAL_DIGITS];
    size_t digit_count = 0;

    do {
        reversed_digits[digit_count++] = (char)('0' + (value % UINT64_C(10)));
        value /= UINT64_C(10);
    } while (value != UINT64_C(0));

    while (digit_count != 0) {
        output[output_length++] = reversed_digits[--digit_count];
    }
    return output_length;
}

static size_t ska_rt_format_i64_line(char output[SKA_RT_I64_LINE_CAPACITY], int64_t value) {
    size_t output_length = 0;
    uint64_t magnitude;

    if (value < 0) {
        /* Unsigned arithmetic avoids overflowing when value is INT64_MIN. */
        magnitude = UINT64_C(0) - (uint64_t)value;
        output[output_length++] = '-';
    } else {
        magnitude = (uint64_t)value;
    }

    output_length = ska_rt_append_u64_decimal(output, output_length, magnitude);
    output[output_length++] = '\n';
    return output_length;
}

static size_t ska_rt_format_u64_line(char output[SKA_RT_UNSIGNED_LINE_CAPACITY], uint64_t value) {
    size_t output_length = ska_rt_append_u64_decimal(output, 0, value);
    output[output_length++] = '\n';
    return output_length;
}

static void ska_rt_format_f64_bits_line(char output[SKA_RT_F64_BITS_LINE_CAPACITY], double value) {
    static const char hexadecimal_digits[] = "0123456789abcdef";
    uint64_t bits;
    size_t digit_index;

    memcpy(&bits, &value, sizeof(bits));
    output[0] = '0';
    output[1] = 'x';
    for (digit_index = 0; digit_index < SKA_RT_F64_HEX_DIGITS; ++digit_index) {
        const size_t shift = (SKA_RT_F64_HEX_DIGITS - 1 - digit_index) * 4;
        output[2 + digit_index] = hexadecimal_digits[(bits >> shift) & UINT64_C(0xf)];
    }
    output[2 + SKA_RT_F64_HEX_DIGITS] = '\n';
}

static _Noreturn void ska_rt_terminate_unsuccessfully(void) {
    /* Runtime boundary failures are unrecoverable. _Exit also avoids an
       implicit attempt to flush stdout after an output failure. */
    _Exit(EXIT_FAILURE);
}

static _Noreturn void ska_rt_runtime_defect(void) {
    abort();
}

static bool ska_rt_write_u64_bytes(FILE* stream, const uint8_t* bytes, uint64_t length) {
    while (length != UINT64_C(0)) {
        size_t chunk_length;

#if SIZE_MAX < UINT64_MAX
        chunk_length = length > (uint64_t)SIZE_MAX ? SIZE_MAX : (size_t)length;
#else
        chunk_length = (size_t)length;
#endif
        if (fwrite(bytes, sizeof(bytes[0]), chunk_length, stream) != chunk_length) {
            return false;
        }
        bytes += chunk_length;
        length -= (uint64_t)chunk_length;
    }
    return true;
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

static void ska_rt_write_stdout_record(const char* record, size_t length) {
    if (!ska_rt_write_u64_bytes(stdout, (const uint8_t*)record, (uint64_t)length)
        || fflush(stdout) == EOF) {
        ska_rt_terminate_unsuccessfully();
    }
}

static void ska_rt_println_unsigned(uint64_t value) {
    char output[SKA_RT_UNSIGNED_LINE_CAPACITY];
    const size_t output_length = ska_rt_format_u64_line(output, value);

    ska_rt_write_stdout_record(output, output_length);
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

void ska_rt_println_i64(int64_t value) {
    char output[SKA_RT_I64_LINE_CAPACITY];
    const size_t output_length = ska_rt_format_i64_line(output, value);

    ska_rt_write_stdout_record(output, output_length);
}

void ska_rt_println_bool(bool value) {
    static const char false_record[] = "false\n";
    static const char true_record[] = "true\n";

    if (value) {
        ska_rt_write_stdout_record(true_record, sizeof(true_record) - 1);
    } else {
        ska_rt_write_stdout_record(false_record, sizeof(false_record) - 1);
    }
}

void ska_rt_println_u64(uint64_t value) {
    ska_rt_println_unsigned(value);
}

void ska_rt_println_u8(uint8_t value) {
    ska_rt_println_unsigned((uint64_t)value);
}

void ska_rt_println_f64_bits(double value) {
    char output[SKA_RT_F64_BITS_LINE_CAPACITY];

    ska_rt_format_f64_bits_line(output, value);
    ska_rt_write_stdout_record(output, sizeof(output));
}
