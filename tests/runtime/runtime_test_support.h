#ifndef SKALD_RUNTIME_TEST_SUPPORT_H
#define SKALD_RUNTIME_TEST_SUPPORT_H

#include <stdint.h>

int runtime_test_report_system_error(const char* operation);
double runtime_test_f64_from_bits(uint64_t bits);

#endif
