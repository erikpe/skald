#define _POSIX_C_SOURCE 200809L

#include "runtime_test_support.h"
#include "skald_runtime.h"

#include <errno.h>
#include <inttypes.h>
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
    CAPTURE_CAPACITY = 32768,
    TRACE_LIMIT = 256
};

enum ExpectedTermination {
    EXPECT_PANIC_EXIT,
    EXPECT_HARD_FAILURE
};

struct ExpectedOutput {
    uint8_t bytes[CAPTURE_CAPACITY];
    size_t length;
};

static const uint8_t LOAD_NAME[] = "app::load";
static const uint8_t LOAD_PATH[] = "app/load.ska";
static const SkaRtTraceContext LOAD_CONTEXT = {
    LOAD_NAME,
    sizeof(LOAD_NAME) - 1,
    LOAD_PATH,
    sizeof(LOAD_PATH) - 1,
};
static const SkaRtTraceLocation LOAD_LOCATION = {
    &LOAD_CONTEXT,
    UINT64_C(18),
    UINT64_C(9),
};
static const SkaRtTraceLocation LOAD_REPLACED_LOCATION = {
    &LOAD_CONTEXT,
    UINT64_MAX,
    UINT64_C(0),
};

static const uint8_t MAIN_NAME[] = "app::main";
static const uint8_t MAIN_PATH[] = "app/main.ska";
static const SkaRtTraceContext MAIN_CONTEXT = {
    MAIN_NAME,
    sizeof(MAIN_NAME) - 1,
    MAIN_PATH,
    sizeof(MAIN_PATH) - 1,
};
static const SkaRtTraceLocation MAIN_LOCATION = {
    &MAIN_CONTEXT,
    UINT64_C(7),
    UINT64_C(5),
};

static const uint8_t BINARY_NAME[] = {'a', 'p', 'p', ':', ':', 'r', 'a', 'w', UINT8_C(0), 'n'};
static const uint8_t BINARY_PATH[] = {'r', 'a', 'w', UINT8_C(0), '.', 's', 'k', 'a'};
static const SkaRtTraceContext BINARY_CONTEXT = {
    BINARY_NAME,
    sizeof(BINARY_NAME),
    BINARY_PATH,
    sizeof(BINARY_PATH),
};
static const SkaRtTraceLocation BINARY_LOCATION = {
    &BINARY_CONTEXT,
    UINT64_C(1),
    UINT64_C(2),
};

static size_t successful_writes_before_failure = SIZE_MAX;

ssize_t __real_write(int descriptor, const void* bytes, size_t length);

ssize_t __wrap_write(int descriptor, const void* bytes, size_t length) {
    if (successful_writes_before_failure == 0) {
        errno = EIO;
        return -1;
    }
    if (successful_writes_before_failure != SIZE_MAX) {
        --successful_writes_before_failure;
    }
    return __real_write(descriptor, bytes, length);
}

static void publish_single_frame(const SkaRtTraceLocation* location,
                                 SkaRtTraceFrame* frame) {
    frame->previous = NULL;
    frame->location = location;
    ska_rt_trace_top = frame;
}

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

static void panic_with_single_frame(void) {
    static const uint8_t message[] = "configuration is missing";
    SkaRtTraceFrame frame;

    publish_single_frame(&LOAD_LOCATION, &frame);
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_with_nested_frames(void) {
    static const uint8_t message[] = "nested";
    SkaRtTraceFrame main_frame = {NULL, &MAIN_LOCATION};
    SkaRtTraceFrame load_frame = {&main_frame, &LOAD_LOCATION};
    SkaRtTraceFrame binary_frame = {&load_frame, &BINARY_LOCATION};

    ska_rt_trace_top = &binary_frame;
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_with_replaced_location(void) {
    static const uint8_t message[] = "replaced";
    SkaRtTraceFrame frame;

    publish_single_frame(&LOAD_LOCATION, &frame);
    frame.location = &LOAD_REPLACED_LOCATION;
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_with_frame_count(size_t count) {
    static const uint8_t message[] = "deep";
    SkaRtTraceFrame frames[TRACE_LIMIT + 1];
    size_t index;

    for (index = 0; index < count; ++index) {
        frames[index].previous = index == 0 ? NULL : &frames[index - 1];
        frames[index].location = &LOAD_LOCATION;
    }
    ska_rt_trace_top = &frames[count - 1];
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_with_256_frames(void) {
    panic_with_frame_count(TRACE_LIMIT);
}

static void panic_with_257_frames(void) {
    panic_with_frame_count(TRACE_LIMIT + 1);
}

static void panic_with_cyclic_chain(void) {
    static const uint8_t message[] = "cycle";
    SkaRtTraceFrame frame = {NULL, &MAIN_LOCATION};

    frame.previous = &frame;
    ska_rt_trace_top = &frame;
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_after_trace_write_failure(void) {
    static const uint8_t message[] = "unwritable trace";
    SkaRtTraceFrame frame;

    publish_single_frame(&LOAD_LOCATION, &frame);
    successful_writes_before_failure = 4;
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

static void panic_with_invalid_null_message(void) {
    ska_rt_panic(NULL, UINT64_C(1));
}

static int append_expected(struct ExpectedOutput* output,
                           const uint8_t* bytes,
                           size_t length) {
    if (length > sizeof(output->bytes) - output->length) {
        fprintf(stderr, "runtime panic expected output exceeded capacity\n");
        return 1;
    }
    if (length != 0) {
        memcpy(output->bytes + output->length, bytes, length);
    }
    output->length += length;
    return 0;
}

static int append_expected_text(struct ExpectedOutput* output, const char* text) {
    return append_expected(output, (const uint8_t*)text, strlen(text));
}

static int append_expected_u64(struct ExpectedOutput* output, uint64_t value) {
    char digits[32];
    const int length = snprintf(digits, sizeof(digits), "%" PRIu64, value);

    if (length < 0 || (size_t)length >= sizeof(digits)) {
        fprintf(stderr, "runtime panic test could not format an expected position\n");
        return 1;
    }
    return append_expected(output, (const uint8_t*)digits, (size_t)length);
}

static int append_expected_frame(struct ExpectedOutput* output,
                                 const SkaRtTraceLocation* location) {
    const SkaRtTraceContext* context = location->context;

    return append_expected_text(output, "  at ")
        || append_expected(output, context->name, (size_t)context->name_length)
        || append_expected_text(output, " (")
        || append_expected(output, context->path, (size_t)context->path_length)
        || append_expected_text(output, ":")
        || append_expected_u64(output, location->line)
        || append_expected_text(output, ":")
        || append_expected_u64(output, location->column)
        || append_expected_text(output, ")\n");
}

static int begin_expected(struct ExpectedOutput* output,
                          const uint8_t* message,
                          size_t message_length,
                          int has_trace) {
    output->length = 0;
    return append_expected_text(output, "panic: ")
        || append_expected(output, message, message_length)
        || append_expected_text(output, "\n")
        || (has_trace && append_expected_text(output, "stacktrace:\n"));
}

static int read_captured_stderr(int descriptor, uint8_t* bytes, size_t* length) {
    for (;;) {
        const ssize_t received = read(descriptor, bytes + *length, CAPTURE_CAPACITY - *length);

        if (received < 0) {
            if (errno == EINTR) {
                continue;
            }
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

static int verify_fixed_records(void) {
    static const uint8_t empty_record[] = "panic: \n";
    static const uint8_t ordinary_record[] = "panic: configuration is missing\n";
    static const uint8_t zero_record[] = {
        'p', 'a', 'n', 'i', 'c', ':', ' ', 'b', 'a', 'd', UINT8_C(0), 'i', 'n', 'p', 'u', 't', '\n'};
    static const uint8_t newline_record[] = "panic: first\nsecond\n";

    return verify_panic("empty message",
                        panic_empty,
                        EXPECT_PANIC_EXIT,
                        empty_record,
                        sizeof(empty_record) - 1)
        || verify_panic("ordinary message",
                        panic_ordinary,
                        EXPECT_PANIC_EXIT,
                        ordinary_record,
                        sizeof(ordinary_record) - 1)
        || verify_panic("embedded-zero message",
                        panic_with_embedded_zero,
                        EXPECT_PANIC_EXIT,
                        zero_record,
                        sizeof(zero_record))
        || verify_panic("embedded-newline message",
                        panic_with_embedded_newline,
                        EXPECT_PANIC_EXIT,
                        newline_record,
                        sizeof(newline_record) - 1);
}

static int verify_trace_records(void) {
    static const uint8_t configuration_message[] = "configuration is missing";
    static const uint8_t nested_message[] = "nested";
    static const uint8_t replaced_message[] = "replaced";
    static const uint8_t deep_message[] = "deep";
    static const uint8_t cycle_message[] = "cycle";
    static const uint8_t failed_message[] = "unwritable trace";
    struct ExpectedOutput expected;
    size_t index;

    if (begin_expected(&expected,
                       configuration_message,
                       sizeof(configuration_message) - 1,
                       1)
        || append_expected_frame(&expected, &LOAD_LOCATION)
        || verify_panic("single trace frame",
                        panic_with_single_frame,
                        EXPECT_PANIC_EXIT,
                        expected.bytes,
                        expected.length)) {
        return 1;
    }

    if (begin_expected(&expected, nested_message, sizeof(nested_message) - 1, 1)
        || append_expected_frame(&expected, &BINARY_LOCATION)
        || append_expected_frame(&expected, &LOAD_LOCATION)
        || append_expected_frame(&expected, &MAIN_LOCATION)
        || verify_panic("nested trace frames",
                        panic_with_nested_frames,
                        EXPECT_PANIC_EXIT,
                        expected.bytes,
                        expected.length)) {
        return 1;
    }

    if (begin_expected(&expected, replaced_message, sizeof(replaced_message) - 1, 1)
        || append_expected_frame(&expected, &LOAD_REPLACED_LOCATION)
        || verify_panic("replaced trace location",
                        panic_with_replaced_location,
                        EXPECT_PANIC_EXIT,
                        expected.bytes,
                        expected.length)) {
        return 1;
    }

    if (begin_expected(&expected, deep_message, sizeof(deep_message) - 1, 1)) {
        return 1;
    }
    for (index = 0; index < TRACE_LIMIT; ++index) {
        if (append_expected_frame(&expected, &LOAD_LOCATION)) {
            return 1;
        }
    }
    if (verify_panic("exactly 256 trace frames",
                     panic_with_256_frames,
                     EXPECT_PANIC_EXIT,
                     expected.bytes,
                     expected.length)) {
        return 1;
    }
    if (append_expected_text(&expected, "  ... outer frames omitted\n")
        || verify_panic("trace longer than 256 frames",
                        panic_with_257_frames,
                        EXPECT_PANIC_EXIT,
                        expected.bytes,
                        expected.length)) {
        return 1;
    }

    if (begin_expected(&expected, cycle_message, sizeof(cycle_message) - 1, 1)) {
        return 1;
    }
    for (index = 0; index < TRACE_LIMIT; ++index) {
        if (append_expected_frame(&expected, &MAIN_LOCATION)) {
            return 1;
        }
    }
    if (append_expected_text(&expected, "  ... outer frames omitted\n")
        || verify_panic("cyclic trace chain",
                        panic_with_cyclic_chain,
                        EXPECT_PANIC_EXIT,
                        expected.bytes,
                        expected.length)) {
        return 1;
    }

    if (begin_expected(&expected, failed_message, sizeof(failed_message) - 1, 1)
        || verify_panic("output failure after trace header",
                        panic_after_trace_write_failure,
                        EXPECT_PANIC_EXIT,
                        expected.bytes,
                        expected.length)) {
        return 1;
    }
    return 0;
}

int main(void) {
    if (verify_fixed_records() || verify_trace_records()) {
        return 1;
    }
    return verify_panic(
        "invalid null message", panic_with_invalid_null_message, EXPECT_HARD_FAILURE, NULL, 0);
}
