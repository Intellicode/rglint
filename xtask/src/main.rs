use clap::Parser;

mod check_parity;
mod gen_docs;
mod port_fixture;

/// xtask entry point. Subcommands live in their own modules; the top-level
/// dispatcher here is just clap plumbing.
#[derive(Debug, Parser)]
#[command(name = "xtask", about = "rglint dev-tooling tasks")]
enum Xtask {
    /// Convert graphql-eslint TS test cases into rglint fixtures (spec-015).
    PortFixture(port_fixture::PortFixtureArgs),
    /// Generate the checked-in rule reference documentation (spec-068).
    GenDocs(gen_docs::GenDocsArgs),
    /// Compare the fixture oracle with rglint (spec-069).
    CheckParity(check_parity::CheckParityArgs),
}

fn main() -> anyhow::Result<()> {
    let cmd = Xtask::parse();
    match cmd {
        Xtask::PortFixture(args) => port_fixture::run(args),
        Xtask::GenDocs(args) => gen_docs::run(args),
        Xtask::CheckParity(args) => check_parity::run(args),
    }
}
