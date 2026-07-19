#ifndef SKALD_RUNTIME_H
#define SKALD_RUNTIME_H

#include <stdint.h>

#define SKALD_RUNTIME_ABI_VERSION UINT64_C(2)

uint64_t ska_rt_abi_version(void);

/* Writes the shortest ASCII decimal representation and one LF to stdout.
   A detected write or flush failure terminates the process unsuccessfully. */
void ska_rt_println_i64(int64_t value);

#endif
