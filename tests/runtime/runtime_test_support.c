#include "runtime_test_support.h"

#include <stdio.h>
#include <string.h>

int runtime_test_report_system_error(const char* operation) {
    perror(operation);
    return 1;
}

double runtime_test_f64_from_bits(uint64_t bits) {
    double value;

    memcpy(&value, &bits, sizeof(value));
    return value;
}
