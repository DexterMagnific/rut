use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cargo-rut", version, about = "Run rut test suites")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a test suite file
    Run(RunArgs),
}

#[derive(Parser)]
pub struct RunArgs {
    /// Path to suite file (or directory for discovery)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Runner type: parallel or sequential
    #[arg(long, value_enum, default_value = "parallel")]
    pub runner: RunnerType,

    /// Max concurrent jobs (parallel runner only)
    #[arg(long, short = 'j')]
    pub jobs: Option<usize>,

    /// Shuffle test case order (parallel runner only)
    #[arg(long)]
    pub shuffle: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum RunnerType {
    Parallel,
    Sequential,
}