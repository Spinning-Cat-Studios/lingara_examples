//! `lingara-examples-check` (ADR 4.10.26b D4): every example installs only
//! what a stranger could install, at exactly the version `examples.toml` says.

mod bump;
mod cli;
mod deps;
mod index;
mod list;
mod outdated;
#[cfg(test)]
mod bump_tests;
#[cfg(test)]
mod cli_tests;
#[cfg(test)]
mod deps_tests;
#[cfg(test)]
mod index_tests;
#[cfg(test)]
mod outdated_tests;
#[cfg(test)]
mod test_support;

fn main() -> std::process::ExitCode {
    cli::run()
}
