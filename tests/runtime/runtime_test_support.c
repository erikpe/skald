#include "runtime_test_support.h"

#include <stdio.h>

int runtime_test_report_system_error(const char* operation) {
    perror(operation);
    return 1;
}
