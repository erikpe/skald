#define _POSIX_C_SOURCE 200809L

#include "runtime_test_support.h"
#include "skald_runtime.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

typedef void (*AllocationFailureScenario)(void);

enum { CHILD_SETUP_FAILURE = 2 };

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

static int verify_failure_is_fatal(const char* description, AllocationFailureScenario scenario) {
    int status;
    const pid_t child = fork();

    if (child < 0) {
        return runtime_test_report_system_error("fork allocation-failure test");
    }
    if (child == 0) {
        scenario();
        _Exit(EXIT_SUCCESS);
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
    return 0;
}

int main(void) {
    if (verify_failure_is_fatal("zero-byte allocation", allocate_zero) != 0) {
        return 1;
    }
#if SIZE_MAX < UINT64_MAX
    if (verify_failure_is_fatal("unrepresentable allocation", allocate_unrepresentable_size)
        != 0) {
        return 1;
    }
#endif
    if (verify_failure_is_fatal("allocation failure", exhaust_address_space) != 0) {
        return 1;
    }
    return 0;
}
