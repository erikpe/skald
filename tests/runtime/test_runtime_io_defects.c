#define _POSIX_C_SOURCE 200809L

#include "runtime_test_support.h"
#include "skald_runtime.h"

#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

typedef void (*IoDefectScenario)(void);

enum { CHILD_SETUP_FAILURE = 2 };

static void invalid_standard_handle_selector(void) {
    (void)ska_rt_io_standard_handle(UINT8_C(3));
}

static void invalid_open_mode(void) {
    (void)ska_rt_io_open(NULL, UINT64_C(0), UINT8_C(1));
}

static void invalid_open_pointer(void) {
    (void)ska_rt_io_open(NULL, UINT64_C(1), UINT8_C(0));
}

static void invalid_read_pointer(void) {
    (void)ska_rt_io_read(STDIN_FILENO, NULL, UINT64_C(1));
}

static void invalid_write_pointer(void) {
    (void)ska_rt_io_write(STDOUT_FILENO, NULL, UINT64_C(1));
}

static void negative_read_handle(void) {
    uint8_t byte;

    (void)ska_rt_io_read(INT64_C(-1), &byte, UINT64_C(1));
}

static void oversized_write_handle(void) {
    static const uint8_t byte = UINT8_C(0);

    (void)ska_rt_io_write((int64_t)INT_MAX + INT64_C(1), &byte, UINT64_C(1));
}

static void negative_close_handle(void) {
    (void)ska_rt_io_close(INT64_C(-1));
}

static int verify_hard_failure(const char* description, IoDefectScenario scenario) {
    int status;
    const pid_t child = fork();

    if (child < 0) {
        return runtime_test_report_system_error("fork runtime I/O defect test");
    }
    if (child == 0) {
        scenario();
        _Exit(EXIT_SUCCESS);
    }
    if (waitpid(child, &status, 0) < 0) {
        return runtime_test_report_system_error("wait for runtime I/O defect test");
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == CHILD_SETUP_FAILURE) {
        fprintf(stderr, "runtime %s child setup failed\n", description);
        return 1;
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == EXIT_SUCCESS) {
        fprintf(stderr, "runtime %s contract defect returned successfully\n", description);
        return 1;
    }
    return 0;
}

int main(void) {
    if (verify_hard_failure("invalid standard-handle selector", invalid_standard_handle_selector)
            != 0
        || verify_hard_failure("invalid open mode", invalid_open_mode) != 0
        || verify_hard_failure("invalid open pointer", invalid_open_pointer) != 0
        || verify_hard_failure("invalid read pointer", invalid_read_pointer) != 0
        || verify_hard_failure("invalid write pointer", invalid_write_pointer) != 0
        || verify_hard_failure("negative read handle", negative_read_handle) != 0
        || verify_hard_failure("oversized write handle", oversized_write_handle) != 0
        || verify_hard_failure("negative close handle", negative_close_handle) != 0) {
        return 1;
    }
    return 0;
}
