fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint_shell::run(std::env::args().skip(1).collect())
}
