#define _POSIX_C_SOURCE 200809L

#include "skald_runtime.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

enum {
    SKA_RT_IO_STANDARD_INPUT = 0,
    SKA_RT_IO_STANDARD_OUTPUT = 1,
    SKA_RT_IO_STANDARD_ERROR = 2,
    SKA_RT_IO_OPEN_READ_EXISTING = 0
};

static _Noreturn void ska_rt_io_contract_defect(void) {
    abort();
}

static int64_t ska_rt_io_error(int error_number) {
    if (error_number <= 0) {
        ska_rt_io_contract_defect();
    }
    return -(int64_t)error_number;
}

static int ska_rt_io_descriptor(int64_t handle) {
    if (handle < INT64_C(0) || handle > INT_MAX) {
        ska_rt_io_contract_defect();
    }
    return (int)handle;
}

static size_t ska_rt_io_transfer_size(uint64_t length) {
    return length > (uint64_t)SSIZE_MAX ? (size_t)SSIZE_MAX : (size_t)length;
}

int64_t ska_rt_io_standard_handle(uint8_t stream) {
    switch (stream) {
        case SKA_RT_IO_STANDARD_INPUT:
            return STDIN_FILENO;
        case SKA_RT_IO_STANDARD_OUTPUT:
            return STDOUT_FILENO;
        case SKA_RT_IO_STANDARD_ERROR:
            return STDERR_FILENO;
        default:
            ska_rt_io_contract_defect();
    }
}

int64_t ska_rt_io_open(const uint8_t* path, uint64_t path_length, uint8_t mode) {
    char* terminated_path;
    size_t path_size;
    int descriptor;

    if (mode != SKA_RT_IO_OPEN_READ_EXISTING || (path == NULL && path_length != UINT64_C(0))) {
        ska_rt_io_contract_defect();
    }
    if (path_length > (uint64_t)(SIZE_MAX - 1)) {
        return ska_rt_io_error(ENAMETOOLONG);
    }

    path_size = (size_t)path_length;
    if (path_size != 0 && memchr(path, '\0', path_size) != NULL) {
        return ska_rt_io_error(EINVAL);
    }
    terminated_path = malloc(path_size + 1);
    if (terminated_path == NULL) {
        return ska_rt_io_error(ENOMEM);
    }
    if (path_size != 0) {
        memcpy(terminated_path, path, path_size);
    }
    terminated_path[path_size] = '\0';

    do {
        descriptor = open(terminated_path, O_RDONLY | O_CLOEXEC);
    } while (descriptor < 0 && errno == EINTR);

    if (descriptor < 0) {
        const int error_number = errno;

        free(terminated_path);
        return ska_rt_io_error(error_number);
    }
    free(terminated_path);
    return descriptor;
}

int64_t ska_rt_io_read(int64_t handle, uint8_t* destination, uint64_t capacity) {
    const int descriptor = ska_rt_io_descriptor(handle);
    ssize_t received;

    if (capacity == UINT64_C(0)) {
        return INT64_C(0);
    }
    if (destination == NULL) {
        ska_rt_io_contract_defect();
    }

    do {
        received = read(descriptor, destination, ska_rt_io_transfer_size(capacity));
    } while (received < 0 && errno == EINTR);

    return received < 0 ? ska_rt_io_error(errno) : (int64_t)received;
}

int64_t ska_rt_io_write(int64_t handle, const uint8_t* source, uint64_t length) {
    const int descriptor = ska_rt_io_descriptor(handle);
    ssize_t written;

    if (length == UINT64_C(0)) {
        return INT64_C(0);
    }
    if (source == NULL) {
        ska_rt_io_contract_defect();
    }

    do {
        written = write(descriptor, source, ska_rt_io_transfer_size(length));
    } while (written < 0 && errno == EINTR);

    return written < 0 ? ska_rt_io_error(errno) : (int64_t)written;
}

int64_t ska_rt_io_close(int64_t handle) {
    const int descriptor = ska_rt_io_descriptor(handle);

    return close(descriptor) < 0 ? ska_rt_io_error(errno) : INT64_C(0);
}
