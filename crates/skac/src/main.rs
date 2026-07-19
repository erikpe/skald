fn main() {
    let exit_code = skald_compiler::driver::run_cli(std::env::args_os());
    std::process::exit(exit_code);
}
