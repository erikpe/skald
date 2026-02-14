Build scripts for the stage-0 compiler.

build.sh
- Run inside WSL2 or Linux
- Usage: ./build.sh path/to/program.ska [output]

Golden stdlib directive
- Golden test sources may include a directive line: // stdlib: module_name
- Multiple modules are allowed: // stdlib: vec_i64,other_module
- For each listed module, the runner prepends stdlib/<module>.ska to the test source before compiling
- Use this to keep reusable library code in stdlib/ and avoid duplicating implementations in golden tests

Notes
- Requires gcc inside WSL2 or Linux.

run_golden.sh
- Run inside WSL2 or Linux
- Usage: ./run_golden.sh
