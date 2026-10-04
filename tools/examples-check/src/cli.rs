//! The command line (ADR 4.10.26b D4). A checking subcommand prints one
//! `<path>: <finding>` line per finding and exits 1 on any; bad input is 2.

use crate::index::{self, Finding, Registry};
use crate::{bump, deps, list, outdated};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "lingara-examples-check", about = "Every example installs only released packages, at the version examples.toml names")]
pub(crate) struct Cli {
    /// The repository root, where examples.toml lives.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// examples.toml against the tree, the recipes and the README's table.
    Index,
    /// Every example's manifest and lock file against its row.
    Deps,
    /// Report each package behind its registry's newest release; never edits.
    Outdated {
        /// A TOML file of base URLs: npm, go, packagist.
        #[arg(long)]
        registry_urls: PathBuf,
    },
    /// Write one package's version into its rows and their manifests.
    Bump { registry: String, name: String, version: String },
    /// One `<path> <language>` line per row, a CI matrix, or the README table.
    List {
        #[arg(long, conflicts_with = "markdown")]
        json: bool,
        #[arg(long)]
        markdown: bool,
    },
    /// One row's language.
    Lang { path: String },
}

/// Nothing to report.
pub const CLEAN: u8 = 0;
/// At least one finding, or `lang` on a path with no row.
pub const FINDINGS: u8 = 1;
/// Input the command cannot read: a missing or malformed file, an unknown
/// registry, a registry that does not answer.
pub const BAD_INPUT: u8 = 2;

pub fn run() -> ExitCode {
    ExitCode::from(exit_code(&Cli::parse()))
}

/// The exit-code contract `make` and every workflow read (D4).
pub(crate) fn exit_code(cli: &Cli) -> u8 {
    dispatch(cli).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        BAD_INPUT
    })
}

fn dispatch(cli: &Cli) -> Result<u8, String> {
    let root = &cli.root;
    match &cli.command {
        Command::Index => Ok(report(index::check(root))),
        Command::Deps => Ok(report(deps::check(root))),
        Command::Outdated { registry_urls } => {
            let urls = outdated::load_urls(registry_urls)?;
            outdated::run(root, &urls, &mut std::io::stdout()).map(|()| CLEAN)
        }
        Command::Bump { registry, name, version } => {
            let registry = Registry::parse(registry).ok_or_else(|| format!("unknown registry `{registry}`"))?;
            bump::run(root, registry, name, version)?.iter().for_each(|p| println!("{p}"));
            Ok(CLEAN)
        }
        Command::List { json, markdown } => {
            let index = index::load(root).map_err(|f| f.to_string())?;
            match (json, markdown) {
                (true, _) => println!("{}", list::json(&index)),
                (_, true) => print!("{}", list::markdown(&index)),
                _ => print!("{}", list::plain(&index)),
            }
            Ok(CLEAN)
        }
        Command::Lang { path } => {
            let index = index::load(root).map_err(|f| f.to_string())?;
            Ok(match list::lang(&index, path) {
                Some(language) => {
                    println!("{language}");
                    CLEAN
                }
                None => report(vec![Finding::new(path, "no row in examples.toml")]),
            })
        }
    }
}

fn report(findings: Vec<Finding>) -> u8 {
    for finding in &findings {
        println!("{finding}");
    }
    if findings.is_empty() { CLEAN } else { FINDINGS }
}
