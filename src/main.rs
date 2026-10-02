fn main() {
    let code = match hippo_cli::cli::run() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            err.exit_code()
        }
    };
    std::process::exit(code);
}
