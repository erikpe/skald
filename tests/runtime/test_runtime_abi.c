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

static int verify_i64_output(void) {
    static const char expected[] =
        "0\n"
        "1\n"
        "-1\n"
        "-9223372036854775808\n"
        "9223372036854775807\n"
        "17\n"
        "-23\n";
    char actual[sizeof(expected)];
    FILE* capture;
    int saved_stdout;
    size_t actual_length;

    if (fflush(stdout) == EOF) {
        return report_system_error("fflush stdout before capture");
    }
    saved_stdout = dup(STDOUT_FILENO);
    if (saved_stdout < 0) {
        return report_system_error("dup stdout");
    }
    capture = tmpfile();
    if (capture == NULL) {
        close(saved_stdout);
        return report_system_error("tmpfile");
    }
    if (dup2(fileno(capture), STDOUT_FILENO) < 0) {
        fclose(capture);
        close(saved_stdout);
        return report_system_error("redirect stdout");
    }

    ska_rt_println_i64(INT64_C(0));
    ska_rt_println_i64(INT64_C(1));
    ska_rt_println_i64(-INT64_C(1));
    ska_rt_println_i64(INT64_MIN);
    ska_rt_println_i64(INT64_MAX);
    ska_rt_println_i64(INT64_C(17));
    ska_rt_println_i64(-INT64_C(23));

    if (dup2(saved_stdout, STDOUT_FILENO) < 0) {
        fclose(capture);
        close(saved_stdout);
        return report_system_error("restore stdout");
    }
    close(saved_stdout);

    if (fseek(capture, 0L, SEEK_SET) != 0) {
        fclose(capture);
        return report_system_error("rewind captured stdout");
    }
    actual_length = fread(actual, sizeof(actual[0]), sizeof(actual), capture);
    if (ferror(capture)) {
        fclose(capture);
        return report_system_error("read captured stdout");
    }
    if (fclose(capture) != 0) {
        return report_system_error("close captured stdout");
    }

    if (actual_length != sizeof(expected) - 1 ||
        memcmp(actual, expected, sizeof(expected) - 1) != 0) {
        fprintf(stderr,
                "runtime i64 output mismatch: expected %zu bytes, received %zu bytes\n",
                sizeof(expected) - 1,
                actual_length);
        return 1;
    }
    return 0;
}

static int verify_output_failure_is_fatal(void) {
    int status;
    const pid_t child = fork();

    if (child < 0) {
        return report_system_error("fork output-failure test");
    }
    if (child == 0) {
        if (close(STDOUT_FILENO) != 0) {
            _Exit(EXIT_FAILURE);
        }
        ska_rt_println_i64(INT64_C(42));
        _Exit(EXIT_SUCCESS);
    }
    if (waitpid(child, &status, 0) < 0) {
        return report_system_error("wait for output-failure test");
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == EXIT_SUCCESS) {
        fprintf(stderr, "runtime output failure returned successfully\n");
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
    if (verify_output_failure_is_fatal() != 0) {
        return 1;
    }

    return 0;
}
