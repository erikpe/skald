#define _POSIX_C_SOURCE 200809L

#include "runtime_test_support.h"
#include "skald_runtime.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

typedef void (*PanicScenario)(void);

enum {
    CHILD_SETUP_FAILURE = 2,
    CAPTURE_CAPACITY = 256
};

enum ExpectedTermination {
    EXPECT_PANIC_EXIT,
    EXPECT_HARD_FAILURE
};

static void panic_empty(void) {
    ska_rt_panic(NULL, UINT64_C(0));
}

static void panic_ordinary(void) {
    static const uint8_t message[] = "configuration is missing";

    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_with_embedded_zero(void) {
    static const uint8_t message[] = {'b', 'a', 'd', UINT8_C(0), 'i', 'n', 'p', 'u', 't'};

    ska_rt_panic(message, (uint64_t)sizeof(message));
}

static void panic_with_embedded_newline(void) {
    static const uint8_t message[] = "first\nsecond";

    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_with_invalid_null_message(void) {
    ska_rt_panic(NULL, UINT64_C(1));
}

static void panic_after_closing_stderr(void) {
    static const uint8_t message[] = "unwritable";

    if (close(STDERR_FILENO) != 0) {
        _Exit(CHILD_SETUP_FAILURE);
    }
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static int read_captured_stderr(int descriptor, uint8_t* bytes, size_t* length) {
    for (;;) {
        const ssize_t received = read(descriptor, bytes + *length, CAPTURE_CAPACITY - *length);

        if (received < 0) {
            return runtime_test_report_system_error("read panic stderr");
        }
        if (received == 0) {
            return 0;
        }
        *length += (size_t)received;
        if (*length == CAPTURE_CAPACITY) {
            fprintf(stderr, "runtime panic stderr exceeded capture capacity\n");
            return 1;
        }
    }
}

static int verify_panic(const char* description,
                        PanicScenario scenario,
                        enum ExpectedTermination expected_termination,
                        const uint8_t* expected_stderr,
                        size_t expected_length) {
    uint8_t actual_stderr[CAPTURE_CAPACITY];
    size_t actual_length = 0;
    int descriptors[2];
    int status;
    pid_t child;

    if (pipe(descriptors) != 0) {
        return runtime_test_report_system_error("pipe panic test");
    }
    child = fork();
    if (child < 0) {
        close(descriptors[0]);
        close(descriptors[1]);
        return runtime_test_report_system_error("fork panic test");
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
        return runtime_test_report_system_error("close panic capture writer");
    }
    if (read_captured_stderr(descriptors[0], actual_stderr, &actual_length) != 0) {
        close(descriptors[0]);
        return 1;
    }
    if (close(descriptors[0]) != 0) {
        return runtime_test_report_system_error("close panic capture reader");
    }
    if (waitpid(child, &status, 0) < 0) {
        return runtime_test_report_system_error("wait for panic test");
    }

    if (WIFEXITED(status) && WEXITSTATUS(status) == CHILD_SETUP_FAILURE) {
        fprintf(stderr, "runtime %s child setup failed\n", description);
        return 1;
    }
    if (expected_termination == EXPECT_PANIC_EXIT
        && (!WIFEXITED(status) || WEXITSTATUS(status) != EXIT_FAILURE)) {
        fprintf(stderr, "runtime %s did not terminate through the panic exit\n", description);
        return 1;
    }
    if (expected_termination == EXPECT_HARD_FAILURE
        && WIFEXITED(status)
        && WEXITSTATUS(status) == EXIT_SUCCESS) {
        fprintf(stderr, "runtime %s contract defect returned successfully\n", description);
        return 1;
    }
    if (actual_length != expected_length
        || (expected_length != 0
            && memcmp(actual_stderr, expected_stderr, expected_length) != 0)) {
        fprintf(stderr,
                "runtime %s stderr mismatch: expected %zu bytes, received %zu bytes\n",
                description,
                expected_length,
                actual_length);
        return 1;
    }
    return 0;
}

int main(void) {
    static const uint8_t empty_record[] = "panic: \n";
    static const uint8_t ordinary_record[] = "panic: configuration is missing\n";
    static const uint8_t zero_record[] = {
        'p', 'a', 'n', 'i', 'c', ':', ' ', 'b', 'a', 'd', UINT8_C(0), 'i', 'n', 'p', 'u', 't', '\n'};
    static const uint8_t newline_record[] = "panic: first\nsecond\n";

    if (verify_panic(
            "empty message",
            panic_empty,
            EXPECT_PANIC_EXIT,
            empty_record,
            sizeof(empty_record) - 1)
        != 0) {
        return 1;
    }
    if (verify_panic(
            "ordinary message",
            panic_ordinary,
            EXPECT_PANIC_EXIT,
            ordinary_record,
            sizeof(ordinary_record) - 1)
        != 0) {
        return 1;
    }
    if (verify_panic(
            "embedded-zero message",
            panic_with_embedded_zero,
            EXPECT_PANIC_EXIT,
            zero_record,
            sizeof(zero_record))
        != 0) {
        return 1;
    }
    if (verify_panic(
            "embedded-newline message",
            panic_with_embedded_newline,
            EXPECT_PANIC_EXIT,
            newline_record,
            sizeof(newline_record) - 1)
        != 0) {
        return 1;
    }
    if (verify_panic(
            "invalid null message",
            panic_with_invalid_null_message,
            EXPECT_HARD_FAILURE,
            NULL,
            0)
        != 0) {
        return 1;
    }
    if (verify_panic(
            "reporter output failure",
            panic_after_closing_stderr,
            EXPECT_PANIC_EXIT,
            NULL,
            0)
        != 0) {
        return 1;
    }
    return 0;
}
