use clap::Parser;

fn main() {
    let code = rglint::cli::run(rglint::cli::Cli::parse());
    std::process::exit(code.code().into());
}
