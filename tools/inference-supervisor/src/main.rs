fn main() {
    let code = match inference_supervisor::main_entry(std::env::args_os().skip(1)) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("inference supervisor: {error}");
            1
        }
    };
    std::process::exit(code);
}
