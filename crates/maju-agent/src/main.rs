fn main() {
    if let Err(e) = maju_agent::run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
