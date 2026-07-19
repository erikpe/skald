#ifndef SKALD_RUNTIME_H
#define SKALD_RUNTIME_H

#include <stdint.h>

#define SKALD_RUNTIME_ABI_VERSION UINT64_C(1)

uint64_t ska_rt_abi_version(void);

#endif

