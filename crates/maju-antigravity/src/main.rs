fn main() {
    if let Err(error) = maju_antigravity::run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
