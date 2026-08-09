#define _POSIX_C_SOURCE 200809L

#include "skald_runtime.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

enum {
    CHILD_SETUP_FAILURE = 2,
    ALLOCATION_ATTEMPTED = 3,
    CAPTURE_CAPACITY = 256
};

static _Noreturn void allocation_attempted(void) {
    _Exit(ALLOCATION_ATTEMPTED);
}

void* __wrap_malloc(size_t size) {
    (void)size;
    allocation_attempted();
}

void* __wrap_calloc(size_t count, size_t size) {
    (void)count;
    (void)size;
    allocation_attempted();
}

void* __wrap_realloc(void* allocation, size_t size) {
    (void)allocation;
    (void)size;
    allocation_attempted();
}

void __wrap_free(void* allocation) {
    (void)allocation;
    allocation_attempted();
}

static _Noreturn void render_trace(void) {
    static const uint8_t message[] = "allocation independent";
    static const uint8_t name[] = "app::main";
    static const uint8_t path[] = "app/main.ska";
    static const SkaRtTraceContext context = {
        name,
        sizeof(name) - 1,
        path,
        sizeof(path) - 1,
    };
    static const SkaRtTraceLocation location = {
        &context,
        UINT64_C(7),
        UINT64_C(5),
    };
    SkaRtTraceFrame frame = {NULL, &location};

    ska_rt_trace_top = &frame;
    ska_rt_panic(message, (uint64_t)(sizeof(message) - 1));
}

int main(void) {
    uint8_t captured[CAPTURE_CAPACITY];
    size_t captured_length = 0;
    int descriptors[2];
    int status;
    pid_t child;

    if (pipe(descriptors) != 0) {
        return EXIT_FAILURE;
    }
    child = fork();
    if (child < 0) {
        return EXIT_FAILURE;
    }
    if (child == 0) {
        if (close(descriptors[0]) != 0
            || dup2(descriptors[1], STDERR_FILENO) < 0
            || close(descriptors[1]) != 0) {
            _Exit(CHILD_SETUP_FAILURE);
        }
        render_trace();
    }

    if (close(descriptors[1]) != 0) {
        return EXIT_FAILURE;
    }
    for (;;) {
        const ssize_t received =
            read(descriptors[0], captured + captured_length, sizeof(captured) - captured_length);

        if (received < 0) {
            return EXIT_FAILURE;
        }
        if (received == 0) {
            break;
        }
        captured_length += (size_t)received;
        if (captured_length == sizeof(captured)) {
            return EXIT_FAILURE;
        }
    }
    if (close(descriptors[0]) != 0 || waitpid(child, &status, 0) < 0) {
        return EXIT_FAILURE;
    }
    if (!WIFEXITED(status) || WEXITSTATUS(status) != EXIT_FAILURE) {
        return EXIT_FAILURE;
    }
    return captured_length == 0 ? EXIT_FAILURE : EXIT_SUCCESS;
}
