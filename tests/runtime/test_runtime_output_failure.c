#define _POSIX_C_SOURCE 200809L

#include "runtime_test_support.h"
#include "skald_runtime.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

typedef void (*OutputScenario)(void);

enum { CHILD_SETUP_FAILURE = 2 };

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
    ska_rt_println_f64_bits(runtime_test_f64_from_bits(UINT64_C(0x3ff0000000000000)));
}

static int verify_output_failure_is_fatal(const char* description, OutputScenario emit) {
    int status;
    const pid_t child = fork();

    if (child < 0) {
        return runtime_test_report_system_error("fork output-failure test");
    }
    if (child == 0) {
        if (close(STDOUT_FILENO) != 0) {
            _Exit(CHILD_SETUP_FAILURE);
        }
        emit();
        _Exit(EXIT_SUCCESS);
    }
    if (waitpid(child, &status, 0) < 0) {
        return runtime_test_report_system_error("wait for output-failure test");
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == CHILD_SETUP_FAILURE) {
        fprintf(stderr, "runtime %s output-failure child setup failed\n", description);
        return 1;
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == EXIT_SUCCESS) {
        fprintf(stderr, "runtime %s output failure returned successfully\n", description);
        return 1;
    }
    return 0;
}

int main(void) {
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
