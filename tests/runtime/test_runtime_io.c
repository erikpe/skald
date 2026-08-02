#define _GNU_SOURCE

#include "runtime_test_support.h"
#include "skald_runtime.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int report_result_mismatch(const char* operation, int64_t expected, int64_t actual) {
    fprintf(stderr,
            "runtime %s result mismatch: expected %lld, received %lld\n",
            operation,
            (long long)expected,
            (long long)actual);
    return 1;
}

static int expect_result(const char* operation, int64_t expected, int64_t actual) {
    return actual == expected ? 0 : report_result_mismatch(operation, expected, actual);
}

static int test_standard_handles(void) {
    if (expect_result("stdin handle", STDIN_FILENO, ska_rt_io_standard_handle(UINT8_C(0))) != 0
        || expect_result("stdout handle", STDOUT_FILENO, ska_rt_io_standard_handle(UINT8_C(1)))
               != 0
        || expect_result("stderr handle", STDERR_FILENO, ska_rt_io_standard_handle(UINT8_C(2)))
               != 0) {
        return 1;
    }
    return 0;
}

static int write_setup_bytes(int descriptor, const uint8_t* bytes, size_t length) {
    size_t offset = 0;

    while (offset != length) {
        const ssize_t written = write(descriptor, bytes + offset, length - offset);

        if (written < 0) {
            if (errno == EINTR) {
                continue;
            }
            return runtime_test_report_system_error("write runtime I/O setup bytes");
        }
        if (written == 0) {
            fprintf(stderr, "runtime I/O setup write made no progress\n");
            return 1;
        }
        offset += (size_t)written;
    }
    return 0;
}

static int test_file_open_read_and_close(void) {
    static const uint8_t expected[] = {'S', 'k', 'a', 'l', 'd', UINT8_C(0), UINT8_C(0xff)};
    char path[] = "/tmp/skald-runtime-io-XXXXXX";
    uint8_t actual[sizeof(expected) + 4];
    const int setup_descriptor = mkstemp(path);
    int64_t handle;
    int flags;

    if (setup_descriptor < 0) {
        return runtime_test_report_system_error("mkstemp runtime I/O test");
    }
    if (write_setup_bytes(setup_descriptor, expected, sizeof(expected)) != 0
        || close(setup_descriptor) != 0) {
        unlink(path);
        return runtime_test_report_system_error("prepare runtime I/O test file");
    }

    handle = ska_rt_io_open((const uint8_t*)path, (uint64_t)strlen(path), UINT8_C(0));
    if (handle < INT64_C(0)) {
        unlink(path);
        return report_result_mismatch("open existing file", INT64_C(0), handle);
    }
    if (unlink(path) != 0) {
        ska_rt_io_close(handle);
        return runtime_test_report_system_error("unlink runtime I/O test file");
    }

    flags = fcntl((int)handle, F_GETFD);
    if (flags < 0 || (flags & FD_CLOEXEC) == 0) {
        ska_rt_io_close(handle);
        fprintf(stderr, "runtime read-only open did not set close-on-exec\n");
        return 1;
    }
    if (expect_result("read binary file",
                      (int64_t)sizeof(expected),
                      ska_rt_io_read(handle, actual, (uint64_t)sizeof(actual)))
            != 0
        || memcmp(actual, expected, sizeof(expected)) != 0) {
        ska_rt_io_close(handle);
        fprintf(stderr, "runtime file read did not preserve exact bytes\n");
        return 1;
    }
    if (expect_result("file EOF", INT64_C(0), ska_rt_io_read(handle, actual, sizeof(actual))) != 0
        || expect_result("close file", INT64_C(0), ska_rt_io_close(handle)) != 0
        || expect_result("read closed file", -EBADF, ska_rt_io_read(handle, actual, sizeof(actual)))
               != 0
        || expect_result("close closed file", -EBADF, ska_rt_io_close(handle)) != 0) {
        return 1;
    }
    return 0;
}

static int test_open_failures(void) {
    static const uint8_t embedded_zero[] = {'b', 'a', 'd', UINT8_C(0), 'p', 'a', 't', 'h'};
    char missing[] = "/tmp/skald-runtime-io-missing-XXXXXX";
    const int missing_descriptor = mkstemp(missing);

    if (missing_descriptor < 0
        || close(missing_descriptor) != 0
        || unlink(missing) != 0) {
        return runtime_test_report_system_error("prepare missing runtime I/O path");
    }

    if (expect_result("missing path",
                      -ENOENT,
                      ska_rt_io_open(
                          (const uint8_t*)missing, (uint64_t)strlen(missing), UINT8_C(0)))
            != 0
        || expect_result("empty path", -ENOENT, ska_rt_io_open(NULL, UINT64_C(0), UINT8_C(0)))
               != 0
        || expect_result("embedded-zero path",
                         -EINVAL,
                         ska_rt_io_open(
                             embedded_zero, (uint64_t)sizeof(embedded_zero), UINT8_C(0)))
               != 0) {
        return 1;
    }
#if SIZE_MAX < UINT64_MAX
    if (expect_result("unrepresentable path",
                      -ENAMETOOLONG,
                      ska_rt_io_open((const uint8_t*)missing,
                                     (uint64_t)SIZE_MAX + UINT64_C(1),
                                     UINT8_C(0)))
        != 0) {
        return 1;
    }
#else
    if (expect_result("unterminatable path",
                      -ENAMETOOLONG,
                      ska_rt_io_open((const uint8_t*)missing, UINT64_MAX, UINT8_C(0)))
        != 0) {
        return 1;
    }
#endif
    return 0;
}

static int test_pipe_read_write_and_eof(void) {
    static const uint8_t expected[] = {'a', UINT8_C(0), 'b', UINT8_C(0xff)};
    uint8_t actual[sizeof(expected) + 5];
    int descriptors[2];

    if (pipe(descriptors) != 0) {
        return runtime_test_report_system_error("pipe runtime I/O transfer test");
    }
    if (expect_result("pipe write",
                      (int64_t)sizeof(expected),
                      ska_rt_io_write(descriptors[1], expected, sizeof(expected)))
            != 0
        || expect_result("partial pipe read",
                         (int64_t)sizeof(expected),
                         ska_rt_io_read(descriptors[0], actual, sizeof(actual)))
               != 0
        || memcmp(actual, expected, sizeof(expected)) != 0) {
        close(descriptors[0]);
        close(descriptors[1]);
        fprintf(stderr, "runtime pipe transfer did not preserve exact bytes\n");
        return 1;
    }
    if (expect_result("close pipe writer", INT64_C(0), ska_rt_io_close(descriptors[1])) != 0
        || expect_result("closed pipe write",
                         -EBADF,
                         ska_rt_io_write(descriptors[1], expected, sizeof(expected)))
               != 0
        || expect_result("pipe EOF", INT64_C(0), ska_rt_io_read(descriptors[0], actual, sizeof(actual)))
               != 0
        || expect_result("close pipe reader", INT64_C(0), ska_rt_io_close(descriptors[0])) != 0
        || expect_result("closed pipe read",
                         -EBADF,
                         ska_rt_io_read(descriptors[0], actual, sizeof(actual)))
               != 0) {
        return 1;
    }
    return 0;
}

static int test_zero_length_transfers(void) {
    if (expect_result("empty read",
                      INT64_C(0),
                      ska_rt_io_read(STDIN_FILENO, NULL, UINT64_C(0)))
            != 0
        || expect_result("empty write",
                         INT64_C(0),
                         ska_rt_io_write(STDOUT_FILENO, NULL, UINT64_C(0)))
               != 0) {
        return 1;
    }
    return 0;
}

static int test_partial_nonblocking_write(void) {
    uint8_t* bytes;
    int descriptors[2];
    int pipe_capacity;
    size_t drained_length;
    int64_t written;

    if (pipe(descriptors) != 0) {
        return runtime_test_report_system_error("pipe runtime partial-write test");
    }
    pipe_capacity = fcntl(descriptors[1], F_GETPIPE_SZ);
    if (pipe_capacity <= 0
        || fcntl(descriptors[1], F_SETFL, fcntl(descriptors[1], F_GETFL) | O_NONBLOCK) < 0) {
        close(descriptors[0]);
        close(descriptors[1]);
        return runtime_test_report_system_error("configure runtime partial-write pipe");
    }
    bytes = malloc((size_t)pipe_capacity + 1);
    if (bytes == NULL) {
        close(descriptors[0]);
        close(descriptors[1]);
        return runtime_test_report_system_error("allocate runtime partial-write bytes");
    }
    memset(bytes, 0xa5, (size_t)pipe_capacity + 1);
    if (write_setup_bytes(descriptors[1], bytes, (size_t)pipe_capacity) != 0) {
        free(bytes);
        close(descriptors[0]);
        close(descriptors[1]);
        return 1;
    }

    drained_length = (size_t)pipe_capacity / 2;
    if (read(descriptors[0], bytes, drained_length) != (ssize_t)drained_length) {
        free(bytes);
        close(descriptors[0]);
        close(descriptors[1]);
        return runtime_test_report_system_error("drain runtime partial-write pipe");
    }
    written = ska_rt_io_write(descriptors[1], bytes, (uint64_t)pipe_capacity + UINT64_C(1));
    if (written <= INT64_C(0) || written >= (int64_t)pipe_capacity + INT64_C(1)) {
        free(bytes);
        close(descriptors[0]);
        close(descriptors[1]);
        fprintf(stderr,
                "runtime partial write did not return partial progress: received %lld of %d\n",
                (long long)written,
                pipe_capacity + 1);
        return 1;
    }

    free(bytes);
    close(descriptors[0]);
    close(descriptors[1]);
    return 0;
}

int main(void) {
    if (test_standard_handles() != 0
        || test_file_open_read_and_close() != 0
        || test_open_failures() != 0
        || test_pipe_read_write_and_eof() != 0
        || test_zero_length_transfers() != 0
        || test_partial_nonblocking_write() != 0) {
        return 1;
    }
    return 0;
}
