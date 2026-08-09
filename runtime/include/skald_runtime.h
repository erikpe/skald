#ifndef SKALD_RUNTIME_H
#define SKALD_RUNTIME_H

/* Public C surface documented in docs/compiler/RUNTIME_ABI.md. */

#include <stdint.h>

#define SKALD_RUNTIME_ABI_VERSION UINT64_C(9)
#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v9

typedef struct SkaRtTraceContext SkaRtTraceContext;
typedef struct SkaRtTraceLocation SkaRtTraceLocation;
typedef struct SkaRtTraceFrame SkaRtTraceFrame;

/* Immutable, length-delimited metadata emitted by the compiler. */
struct SkaRtTraceContext {
    const uint8_t* name;
    uint64_t name_length;
    const uint8_t* path;
    uint64_t path_length;
};

struct SkaRtTraceLocation {
    const SkaRtTraceContext* context;
    uint64_t line;
    uint64_t column;
};

/* Live native-frame record maintained by compiler-generated code. */
struct SkaRtTraceFrame {
    SkaRtTraceFrame* previous;
    const SkaRtTraceLocation* location;
};

/* Compiler/runtime-private trace state. This is a direct TLS dependency, not
   a source-callable ABI entry point. */
#if defined(__GNUC__) || defined(__clang__)
#define SKA_RT_HIDDEN __attribute__((visibility("hidden")))
#else
#error "Skald runtime requires compiler support for hidden ELF visibility"
#endif
extern SKA_RT_HIDDEN _Thread_local SkaRtTraceFrame* ska_rt_trace_top;
#undef SKA_RT_HIDDEN

/* Version-specific link guard required by compiler-generated executables. */
void SKALD_RUNTIME_ABI_MARKER(void);

/* Runtime inspection hook; link compatibility uses SKALD_RUNTIME_ABI_MARKER. */
uint64_t ska_rt_abi_version(void);

/* Allocates byte_count suitably aligned bytes. Zero or unrepresentable counts
   are runtime defects; host exhaustion reports "memory allocation failed". */
void* ska_rt_alloc(uint64_t byte_count);

/* Releases the exact base pointer returned by one successful ska_rt_alloc. */
void ska_rt_free(void* allocation);

/* Writes the length-delimited panic record and any active runtime trace
   directly to stderr, then terminates unsuccessfully. bytes may be NULL only
   when length is zero. */
_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length);

/* Returns the raw POSIX descriptor for stdin (0), stdout (1), or stderr (2).
   Any other selector is a runtime contract defect. */
int64_t ska_rt_io_standard_handle(uint8_t stream);

/* Opens a length-delimited raw pathname. Mode zero opens an existing path
   read-only with close-on-exec; any other mode is a contract defect. */
int64_t ska_rt_io_open(const uint8_t* path, uint64_t path_length, uint8_t mode);

/* Performs at most one successful POSIX transfer, retrying interruption before
   progress. A zero length permits a null pointer and returns zero. */
int64_t ska_rt_io_read(int64_t handle, uint8_t* destination, uint64_t capacity);
int64_t ska_rt_io_write(int64_t handle, const uint8_t* source, uint64_t length);

/* Attempts one close. The handle must be representable as a POSIX descriptor. */
int64_t ska_rt_io_close(int64_t handle);

#endif
