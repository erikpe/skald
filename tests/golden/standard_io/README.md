# Standard I/O fixtures

The specs in this directory own standard stream and file-read observations.
Small textual streams are inline, while byte payloads are external files. The
file-read success case receives named files in its private run directory;
directory-error coverage declares the shared `read_files/` fixture as an
explicit read-only working directory.

Run this group with `scripts/golden.sh --filter 'standard_io/**'`, or select
only primitive printing with `scripts/golden.sh --filter
'standard_io/printing**'`.
