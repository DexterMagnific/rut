mod build;
mod cli;
mod discovery;
mod error;
mod parser;
mod wrapper;

use crate::cli::{Cli, Commands, RunArgs};
use crate::discovery::discover_suite_file;
use crate::error::{Result, RutError};
use crate::parser::extract_typename;
use crate::wrapper::generate_wrapper;
use clap::Parser;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cli = Cli::parse_from(&args[1..]);

    match cli.command {
        Commands::Run(args) => run_command(args),
    }
}

fn run_command(args: RunArgs) -> Result<()> {
    let workspace_root = find_workspace_root();

    let suite_file = discover_suite_file(args.path)?;
    eprintln!("Found suite file: {}", suite_file.display());

    let content = std::fs::read_to_string(&suite_file)
        .map_err(|e| RutError::ParseError(format!("failed to read {}: {}", suite_file.display(), e)))?;

    let typename = extract_typename(&content)?;
    eprintln!("Found suite typename: {}", typename);

    let wrapper = generate_wrapper(
        &suite_file,
        &typename,
        args.runner,
        args.jobs,
        args.shuffle,
    );

    let status = build::run_temp_project(&workspace_root, wrapper)?;

    std::process::exit(status.code().unwrap_or(1));
}

fn find_workspace_root() -> std::path::PathBuf {
    let mut current = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return current;
                }
            }
        }
        if !current.pop() {
            break;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}