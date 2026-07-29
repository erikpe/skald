#define _POSIX_C_SOURCE 200809L

#include "runtime_test_support.h"
#include "skald_runtime.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

typedef void (*AllocationFailureScenario)(void);

enum { CHILD_SETUP_FAILURE = 2 };
enum { CAPTURE_CAPACITY = 128 };

static void allocate_zero(void) {
    (void)ska_rt_alloc(UINT64_C(0));
}

static void exhaust_address_space(void) {
    struct rlimit limit;

    if (getrlimit(RLIMIT_AS, &limit) != 0) {
        _Exit(CHILD_SETUP_FAILURE);
    }
    limit.rlim_cur = 1;
    if (setrlimit(RLIMIT_AS, &limit) != 0) {
        _Exit(CHILD_SETUP_FAILURE);
    }
    (void)ska_rt_alloc(UINT64_C(1) << 30);
}

#if SIZE_MAX < UINT64_MAX
static void allocate_unrepresentable_size(void) {
    (void)ska_rt_alloc((uint64_t)SIZE_MAX + UINT64_C(1));
}
#endif

static int verify_failure(const char* description,
                          AllocationFailureScenario scenario,
                          const char* expected_stderr) {
    unsigned char actual_stderr[CAPTURE_CAPACITY];
    size_t actual_length = 0;
    int descriptors[2];
    int status;
    pid_t child;

    if (pipe(descriptors) != 0) {
        return runtime_test_report_system_error("pipe allocation-failure test");
    }
    child = fork();
    if (child < 0) {
        close(descriptors[0]);
        close(descriptors[1]);
        return runtime_test_report_system_error("fork allocation-failure test");
    }
    if (child == 0) {
        if (close(descriptors[0]) != 0
            || dup2(descriptors[1], STDERR_FILENO) < 0
            || close(descriptors[1]) != 0) {
            _Exit(CHILD_SETUP_FAILURE);
        }
        scenario();
        _Exit(EXIT_SUCCESS);
    }
    if (close(descriptors[1]) != 0) {
        close(descriptors[0]);
        return runtime_test_report_system_error("close allocation capture writer");
    }
    for (;;) {
        const ssize_t received = read(descriptors[0],
                                      actual_stderr + actual_length,
                                      CAPTURE_CAPACITY - actual_length);
        if (received < 0) {
            close(descriptors[0]);
            return runtime_test_report_system_error("read allocation stderr");
        }
        if (received == 0) {
            break;
        }
        actual_length += (size_t)received;
        if (actual_length == CAPTURE_CAPACITY) {
            fprintf(stderr, "runtime allocation stderr exceeded capture capacity\n");
            close(descriptors[0]);
            return 1;
        }
    }
    if (close(descriptors[0]) != 0) {
        return runtime_test_report_system_error("close allocation capture reader");
    }
    if (waitpid(child, &status, 0) < 0) {
        return runtime_test_report_system_error("wait for allocation-failure test");
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == CHILD_SETUP_FAILURE) {
        fprintf(stderr, "runtime %s child setup failed\n", description);
        return 1;
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == EXIT_SUCCESS) {
        fprintf(stderr, "runtime %s returned successfully\n", description);
        return 1;
    }
    {
        const size_t expected_length = strlen(expected_stderr);
        if (actual_length != expected_length
            || memcmp(actual_stderr, expected_stderr, expected_length) != 0) {
            fprintf(stderr, "runtime %s produced unexpected stderr\n", description);
            return 1;
        }
    }
    return 0;
}

int main(void) {
    if (verify_failure("zero-byte allocation", allocate_zero, "") != 0) {
        return 1;
    }
#if SIZE_MAX < UINT64_MAX
    if (verify_failure("unrepresentable allocation", allocate_unrepresentable_size, "") != 0) {
        return 1;
    }
#endif
    if (verify_failure("allocation failure",
                       exhaust_address_space,
                       "panic: memory allocation failed\n")
        != 0) {
        return 1;
    }
    return 0;
}
