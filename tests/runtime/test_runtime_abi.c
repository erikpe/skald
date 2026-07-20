#define _POSIX_C_SOURCE 200809L

#include "skald_runtime.h"

#include <float.h>
#include <inttypes.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

static int report_system_error(const char* operation) {
    perror(operation);
    return 1;
}

typedef void (*OutputScenario)(void);

#if !defined(__STDC_IEC_559__) || __STDC_IEC_559__ != 1
#error "runtime test requires IEC 60559 / IEEE-754 floating-point semantics"
#endif

_Static_assert(CHAR_BIT == 8, "runtime test requires eight-bit bytes");
_Static_assert(SKALD_RUNTIME_ABI_VERSION == UINT64_C(4), "T1 requires runtime ABI version 4");
_Static_assert(sizeof(double) == sizeof(uint64_t), "runtime test requires a 64-bit double");
_Static_assert(FLT_RADIX == 2, "runtime test requires a binary double");
_Static_assert(DBL_MANT_DIG == 53, "runtime test requires an IEEE-754 binary64 significand");
_Static_assert(DBL_MIN_EXP == -1021, "runtime test requires the binary64 exponent range");
_Static_assert(DBL_MAX_EXP == 1024, "runtime test requires the binary64 exponent range");

static int verify_exact_stdout(const char* description,
                               const char* expected,
                               size_t expected_length,
                               OutputScenario emit) {
    char* actual;
    FILE* capture;
    int saved_stdout;
    size_t actual_length;

    actual = malloc(expected_length + 1);
    if (actual == NULL) {
        return report_system_error("allocate stdout capture");
    }
    if (fflush(stdout) == EOF) {
        free(actual);
        return report_system_error("fflush stdout before capture");
    }
    saved_stdout = dup(STDOUT_FILENO);
    if (saved_stdout < 0) {
        free(actual);
        return report_system_error("dup stdout");
    }
    capture = tmpfile();
    if (capture == NULL) {
        close(saved_stdout);
        free(actual);
        return report_system_error("tmpfile");
    }
    if (dup2(fileno(capture), STDOUT_FILENO) < 0) {
        fclose(capture);
        close(saved_stdout);
        free(actual);
        return report_system_error("redirect stdout");
    }

    emit();

    /* Runtime output operations flush their complete records. Restore the
       process descriptor before inspecting the temporary stream. */
    if (dup2(saved_stdout, STDOUT_FILENO) < 0) {
        fclose(capture);
        close(saved_stdout);
        free(actual);
        return report_system_error("restore stdout");
    }
    close(saved_stdout);

    if (fseek(capture, 0L, SEEK_SET) != 0) {
        fclose(capture);
        free(actual);
        return report_system_error("rewind captured stdout");
    }
    actual_length = fread(actual, sizeof(actual[0]), expected_length + 1, capture);
    if (ferror(capture)) {
        fclose(capture);
        free(actual);
        return report_system_error("read captured stdout");
    }
    if (fclose(capture) != 0) {
        free(actual);
        return report_system_error("close captured stdout");
    }

    if (actual_length != expected_length || memcmp(actual, expected, expected_length) != 0) {
        fprintf(stderr,
                "runtime %s output mismatch: expected %zu bytes, received %zu bytes\n",
                description,
                expected_length,
                actual_length);
        free(actual);
        return 1;
    }
    free(actual);
    return 0;
}

static void emit_i64_output_cases(void) {
    ska_rt_println_i64(INT64_C(0));
    ska_rt_println_i64(INT64_C(1));
    ska_rt_println_i64(-INT64_C(1));
    ska_rt_println_i64(INT64_MIN);
    ska_rt_println_i64(INT64_MAX);
    ska_rt_println_i64(INT64_C(17));
    ska_rt_println_i64(-INT64_C(23));
}

static int verify_i64_output(void) {
    static const char expected[] =
        "0\n"
        "1\n"
        "-1\n"
        "-9223372036854775808\n"
        "9223372036854775807\n"
        "17\n"
        "-23\n";

    return verify_exact_stdout("i64", expected, sizeof(expected) - 1, emit_i64_output_cases);
}

static void emit_bool_output_cases(void) {
    ska_rt_println_bool(false);
    ska_rt_println_bool(true);
    ska_rt_println_bool(false);
    ska_rt_println_bool(true);
}

static int verify_bool_output(void) {
    static const char expected[] =
        "false\n"
        "true\n"
        "false\n"
        "true\n";

    return verify_exact_stdout("bool", expected, sizeof(expected) - 1, emit_bool_output_cases);
}

static void emit_u64_output_cases(void) {
    ska_rt_println_u64(UINT64_C(0));
    ska_rt_println_u64(UINT64_C(1));
    ska_rt_println_u64(UINT64_C(17));
    ska_rt_println_u64(UINT64_MAX);
}

static int verify_u64_output(void) {
    static const char expected[] =
        "0\n"
        "1\n"
        "17\n"
        "18446744073709551615\n";

    return verify_exact_stdout("u64", expected, sizeof(expected) - 1, emit_u64_output_cases);
}

static void emit_u8_output_cases(void) {
    ska_rt_println_u8(UINT8_C(0));
    ska_rt_println_u8(UINT8_C(1));
    ska_rt_println_u8(UINT8_C(17));
    ska_rt_println_u8(UINT8_MAX);
}

static int verify_u8_output(void) {
    static const char expected[] =
        "0\n"
        "1\n"
        "17\n"
        "255\n";

    return verify_exact_stdout("u8", expected, sizeof(expected) - 1, emit_u8_output_cases);
}

static double f64_from_bits(uint64_t bits) {
    double value;

    memcpy(&value, &bits, sizeof(value));
    return value;
}

static void emit_f64_bits_output_cases(void) {
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x0000000000000000)));
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x8000000000000000)));
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x3ff8000000000000)));
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0xc002000000000000)));
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x0000000000000001)));
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x7fefffffffffffff)));
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x7ff0000000000000)));
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x7ff8000000000042)));
}

static int verify_f64_bits_output(void) {
    static const char expected[] =
        "0x0000000000000000\n"
        "0x8000000000000000\n"
        "0x3ff8000000000000\n"
        "0xc002000000000000\n"
        "0x0000000000000001\n"
        "0x7fefffffffffffff\n"
        "0x7ff0000000000000\n"
        "0x7ff8000000000042\n";

    return verify_exact_stdout(
        "f64 bits", expected, sizeof(expected) - 1, emit_f64_bits_output_cases);
}

static void emit_mixed_output_cases(void) {
    ska_rt_println_u64(UINT64_MAX);
    ska_rt_println_bool(true);
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x3ff0000000000000)));
    ska_rt_println_i64(-INT64_C(1));
    ska_rt_println_u8(UINT8_MAX);
}

static int verify_mixed_output(void) {
    static const char expected[] =
        "18446744073709551615\n"
        "true\n"
        "0x3ff0000000000000\n"
        "-1\n"
        "255\n";

    return verify_exact_stdout("mixed", expected, sizeof(expected) - 1, emit_mixed_output_cases);
}

static void emit_i64_failure_case(void) {
    ska_rt_println_i64(INT64_C(42));
}

static void emit_bool_failure_case(void) {
    ska_rt_println_bool(true);
}

static void emit_u64_failure_case(void) {
    ska_rt_println_u64(UINT64_MAX);
}

static void emit_u8_failure_case(void) {
    ska_rt_println_u8(UINT8_MAX);
}

static void emit_f64_bits_failure_case(void) {
    ska_rt_println_f64_bits(f64_from_bits(UINT64_C(0x3ff0000000000000)));
}

static int verify_output_failure_is_fatal(const char* description, OutputScenario emit) {
    int status;
    const pid_t child = fork();

    if (child < 0) {
        return report_system_error("fork output-failure test");
    }
    if (child == 0) {
        if (close(STDOUT_FILENO) != 0) {
            _Exit(EXIT_FAILURE);
        }
        emit();
        _Exit(EXIT_SUCCESS);
    }
    if (waitpid(child, &status, 0) < 0) {
        return report_system_error("wait for output-failure test");
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == EXIT_SUCCESS) {
        fprintf(stderr, "runtime %s output failure returned successfully\n", description);
        return 1;
    }
    return 0;
}

int main(void) {
    const uint64_t reported_version = ska_rt_abi_version();

    if (reported_version != SKALD_RUNTIME_ABI_VERSION) {
        fprintf(stderr,
                "runtime ABI version mismatch: header=%" PRIu64 ", runtime=%" PRIu64 "\n",
                SKALD_RUNTIME_ABI_VERSION,
                reported_version);
        return 1;
    }

    if (verify_i64_output() != 0) {
        return 1;
    }
    if (verify_bool_output() != 0) {
        return 1;
    }
    if (verify_u64_output() != 0) {
        return 1;
    }
    if (verify_u8_output() != 0) {
        return 1;
    }
    if (verify_f64_bits_output() != 0) {
        return 1;
    }
    if (verify_mixed_output() != 0) {
        return 1;
    }
    if (verify_output_failure_is_fatal("i64", emit_i64_failure_case) != 0) {
        return 1;
    }
    if (verify_output_failure_is_fatal("bool", emit_bool_failure_case) != 0) {
        return 1;
    }
    if (verify_output_failure_is_fatal("u64", emit_u64_failure_case) != 0) {
        return 1;
    }
    if (verify_output_failure_is_fatal("u8", emit_u8_failure_case) != 0) {
        return 1;
    }
    if (verify_output_failure_is_fatal("f64 bits", emit_f64_bits_failure_case) != 0) {
        return 1;
    }

    return 0;
}
