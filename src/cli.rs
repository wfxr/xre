use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{ArgMatches, Parser, ValueEnum};

#[derive(Parser)]
#[clap(about, version)]
#[clap(disable_help_subcommand = true)]
#[clap(after_help = "Input is processed line by line. Multi-line patterns are not supported.")]
#[clap(styles(Styles::styled()
    .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
    .usage(AnsiColor::Yellow.on_default() | Effects::BOLD)
    .literal(AnsiColor::Green.on_default() | Effects::BOLD)
    .placeholder(AnsiColor::Cyan.on_default())
))]
pub struct App {
    /// Extract pattern. Can be specified multiple times.
    /// Earlier patterns have higher priority.
    #[arg(short = 'e', long = "extract", id = "expressions", value_name = "PATTERN", action = clap::ArgAction::Append)]
    expressions_raw: Vec<String>,

    /// Replacement template for the preceding -e pattern.
    /// Uses capture group syntax ($1, $2, ...).
    #[arg(short = 'r', long = "replace", id = "replacements", value_name = "REPLACEMENT", action = clap::ArgAction::Append)]
    replacements_raw: Vec<String>,

    /// Sort strategy for output.
    #[arg(short, long, value_enum, default_value_t = SortStrategy::Appearance)]
    pub sort: SortStrategy,

    /// Disable deduplication.
    #[arg(long)]
    pub no_dedup: bool,

    /// Prefix each match with its line number.
    #[arg(short = 'n', long)]
    pub line_number: bool,

    /// Strip ANSI escape sequences from input.
    #[arg(long)]
    pub strip_ansi: bool,

    /// Input file (defaults to stdin).
    #[arg(value_hint = clap::ValueHint::FilePath)]
    pub file: Option<PathBuf>,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum SortStrategy {
    Appearance,
    Frequency,
    Alpha,
}

/// A parsed extraction rule from CLI arguments.
pub struct ParsedExpression {
    pub pattern: String,
    pub replacement: Option<String>,
}

/// Parse `-e` and `-r` flags into paired expressions using argv index ordering.
///
/// Each `-r` binds to the immediately preceding `-e`.
/// Errors if `-r` appears before any `-e`, or two `-r` in a row target the same `-e`.
pub fn parse_expressions(matches: &ArgMatches) -> anyhow::Result<Vec<ParsedExpression>> {
    let e_entries: Vec<(usize, String)> = matches
        .indices_of("expressions")
        .zip(matches.get_many::<String>("expressions"))
        .map(|(indices, values)| indices.zip(values.cloned()).collect())
        .unwrap_or_default();

    let r_entries: Vec<(usize, String)> = matches
        .indices_of("replacements")
        .zip(matches.get_many::<String>("replacements"))
        .map(|(indices, values)| indices.zip(values.cloned()).collect())
        .unwrap_or_default();

    enum Token {
        Pattern(String),
        Replacement(String),
    }

    let mut tokens: Vec<(usize, Token)> = Vec::new();
    for (idx, val) in e_entries {
        tokens.push((idx, Token::Pattern(val)));
    }
    for (idx, val) in r_entries {
        tokens.push((idx, Token::Replacement(val)));
    }
    tokens.sort_by_key(|(idx, _)| *idx);

    let mut rules: Vec<ParsedExpression> = Vec::new();
    for (_, token) in tokens {
        match token {
            Token::Pattern(p) => rules.push(ParsedExpression { pattern: p, replacement: None }),
            Token::Replacement(r) => {
                let last = rules
                    .last_mut()
                    .ok_or_else(|| anyhow::anyhow!("-r/--replace must follow a -e/--extract"))?;
                if last.replacement.is_some() {
                    anyhow::bail!("duplicate -r/--replace for pattern '{}'", last.pattern);
                }
                last.replacement = Some(r);
            }
        }
    }

    Ok(rules)
}
