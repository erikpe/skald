#ifndef SKALD_RUNTIME_H
#define SKALD_RUNTIME_H

/* Public C surface documented in docs/compiler/RUNTIME_ABI.md. */

#include <stdbool.h>
#include <stdint.h>

#define SKALD_RUNTIME_ABI_VERSION UINT64_C(7)
#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v7

/* Version-specific link guard required by compiler-generated executables. */
void SKALD_RUNTIME_ABI_MARKER(void);

/* Runtime inspection hook; link compatibility uses SKALD_RUNTIME_ABI_MARKER. */
uint64_t ska_rt_abi_version(void);

/* Allocates byte_count suitably aligned bytes. Zero or unrepresentable counts
   are runtime defects; host exhaustion reports "memory allocation failed". */
void* ska_rt_alloc(uint64_t byte_count);

/* Releases the exact base pointer returned by one successful ska_rt_alloc. */
void ska_rt_free(void* allocation);

/* Writes "panic: ", exactly length message bytes, and one LF directly to
   stderr, then terminates unsuccessfully. bytes may be NULL only when length
   is zero. */
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

/* Writes the shortest ASCII decimal representation and one LF to stdout.
   A detected write or flush failure terminates the process unsuccessfully. */
void ska_rt_println_i64(int64_t value);

/* Writes lowercase "true" or "false" and one LF to stdout.
   A detected write or flush failure terminates the process unsuccessfully. */
void ska_rt_println_bool(bool value);

/* Writes the shortest unsigned ASCII decimal representation and one LF.
   A detected write or flush failure terminates the process unsuccessfully. */
void ska_rt_println_u64(uint64_t value);
void ska_rt_println_u8(uint8_t value);

/* Writes "0x", exactly 16 lowercase hexadecimal digits containing the
   IEEE-754 binary64 representation, and one LF. A detected write or flush
   failure terminates the process unsuccessfully. */
void ska_rt_println_f64_bits(double value);

#endif
