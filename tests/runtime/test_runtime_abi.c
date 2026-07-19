#define _POSIX_C_SOURCE 200809L

#include "skald_runtime.h"

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

static void emit_i64_failure_case(void) {
    ska_rt_println_i64(INT64_C(42));
}

static void emit_bool_failure_case(void) {
    ska_rt_println_bool(true);
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
    if (verify_output_failure_is_fatal("i64", emit_i64_failure_case) != 0) {
        return 1;
    }
    if (verify_output_failure_is_fatal("bool", emit_bool_failure_case) != 0) {
        return 1;
    }

    return 0;
}
