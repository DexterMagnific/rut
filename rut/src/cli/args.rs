use anyhow::{Context, Result, anyhow};
use std::str::FromStr;

/// How many values a declared argument accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// A boolean switch that takes no value.
    Flag,
    /// A single value, the last occurrence wins.
    Value,
    /// A repeatable value, every occurrence is kept.
    Multi,
}

/// A command line argument declared by a runner or reporter plugin.
///
/// Specs are backend agnostic so plugins do not need to depend on the argument
/// parser used by the harness.
#[derive(Clone, Debug)]
pub struct ArgSpec {
    pub long: String,
    pub short: Option<char>,
    pub kind: ArgKind,
    pub value_name: Option<String>,
    pub help: String,
    pub default: Option<String>,
    pub conflicts_with: Vec<String>,
}

impl ArgSpec {
    fn new(long: impl Into<String>, kind: ArgKind) -> Self {
        Self {
            long: long.into(),
            short: None,
            kind,
            value_name: None,
            help: String::new(),
            default: None,
            conflicts_with: Vec::new(),
        }
    }

    /// Declares a boolean switch, such as `--shuffle`.
    pub fn flag(long: impl Into<String>) -> Self {
        Self::new(long, ArgKind::Flag)
    }

    /// Declares a single valued argument, such as `--jobs 4`.
    pub fn value(long: impl Into<String>) -> Self {
        Self::new(long, ArgKind::Value)
    }

    /// Declares a repeatable valued argument, such as `--filter`.
    pub fn multi(long: impl Into<String>) -> Self {
        Self::new(long, ArgKind::Multi)
    }

    pub fn short(mut self, short: char) -> Self {
        self.short = Some(short);
        self
    }

    pub fn value_name(mut self, value_name: impl Into<String>) -> Self {
        self.value_name = Some(value_name.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    pub fn default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }

    /// Rejects this argument when the named argument is also present.
    pub fn conflicts_with(mut self, long: impl Into<String>) -> Self {
        self.conflicts_with.push(long.into());
        self
    }
}

/// The parsed values of the arguments a plugin declared.
///
/// Lookups use the long name of the argument and never panic: an argument that
/// was not declared simply reads as absent.
pub struct PluginArgs<'a> {
    matches: &'a clap::ArgMatches,
}

impl<'a> PluginArgs<'a> {
    pub(crate) fn new(matches: &'a clap::ArgMatches) -> Self {
        Self { matches }
    }

    /// Returns whether a boolean switch was given.
    pub fn flag(&self, long: &str) -> bool {
        self.matches
            .try_get_one::<bool>(long)
            .ok()
            .flatten()
            .copied()
            .unwrap_or(false)
    }

    /// Returns whether the argument was supplied on the command line.
    ///
    /// An argument that only holds its declared default reads as absent.
    pub fn is_present(&self, long: &str) -> bool {
        matches!(
            self.matches.value_source(long),
            Some(clap::parser::ValueSource::CommandLine)
        )
    }

    /// Returns the value of a single valued argument.
    pub fn value(&self, long: &str) -> Option<&str> {
        self.matches
            .try_get_one::<String>(long)
            .ok()
            .flatten()
            .map(String::as_str)
    }

    /// Returns every value of a repeatable argument, in command line order.
    pub fn values(&self, long: &str) -> Vec<&str> {
        self.matches
            .try_get_many::<String>(long)
            .ok()
            .flatten()
            .map(|values| values.map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Parses the value of a single valued argument.
    pub fn parsed<T>(&self, long: &str) -> Result<Option<T>>
    where
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        match self.value(long) {
            None => Ok(None),
            Some(raw) => raw
                .parse::<T>()
                .map(Some)
                .map_err(|error| anyhow!("invalid value for --{long}: {error}")),
        }
    }

    /// Parses the value of a single valued argument, requiring it to be present.
    pub fn required<T>(&self, long: &str) -> Result<T>
    where
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        self.parsed(long)?
            .with_context(|| format!("--{long} is required"))
    }
}

pub(crate) fn to_clap_arg(spec: &ArgSpec) -> clap::Arg {
    let mut arg = clap::Arg::new(spec.long.clone()).long(spec.long.clone());

    if let Some(short) = spec.short {
        arg = arg.short(short);
    }
    if !spec.help.is_empty() {
        arg = arg.help(spec.help.clone());
    }

    arg = match spec.kind {
        ArgKind::Flag => arg.action(clap::ArgAction::SetTrue),
        ArgKind::Value => arg.action(clap::ArgAction::Set),
        ArgKind::Multi => arg.action(clap::ArgAction::Append),
    };

    if spec.kind != ArgKind::Flag {
        arg = arg.value_name(
            spec.value_name
                .clone()
                .unwrap_or_else(|| spec.long.to_uppercase().replace('-', "_")),
        );
    }
    if let Some(default) = &spec.default {
        arg = arg.default_value(default.clone());
    }
    for conflict in &spec.conflicts_with {
        arg = arg.conflicts_with(conflict.clone());
    }

    arg
}
