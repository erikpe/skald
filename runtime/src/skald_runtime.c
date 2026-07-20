#include "skald_runtime.h"

#include <float.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

static void ska_rt_output_failure(void) {
    /* Output failure is unrecoverable at the bootstrap ABI. _Exit avoids a
       second implicit attempt to flush the already-failed stdout stream. */
    _Exit(EXIT_FAILURE);
}

static void ska_rt_write_stdout_record(const char* record, size_t length) {
    if (fwrite(record, sizeof(record[0]), length, stdout) != length || fflush(stdout) == EOF) {
        ska_rt_output_failure();
    }
}

static void ska_rt_println_unsigned(uint64_t value) {
    char output[SKA_RT_UNSIGNED_LINE_CAPACITY];
    const size_t output_length = ska_rt_format_u64_line(output, value);

    ska_rt_write_stdout_record(output, output_length);
}

uint64_t ska_rt_abi_version(void) {
    return SKALD_RUNTIME_ABI_VERSION;
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
