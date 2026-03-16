use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::process;

use anyhow::{Context, bail};
use clap::{CommandFactory, FromArgMatches};
use regex::Regex;

mod cli;

use cli::App;
use xre::{ExtractRule, SortStrategy, extract, strip_ansi};

fn main() {
    if let Err(e) = try_main() {
        if let Some(ioerr) = e.root_cause().downcast_ref::<io::Error>()
            && ioerr.kind() == io::ErrorKind::BrokenPipe
        {
            process::exit(0);
        }
        eprintln!("{}: {e}", env!("CARGO_PKG_NAME"));
        process::exit(1);
    }
}

fn try_main() -> anyhow::Result<()> {
    let matches = App::command().get_matches();
    let app = App::from_arg_matches(&matches)?;
    let expressions = cli::parse_expressions(&matches)?;

    if expressions.is_empty() {
        bail!("at least one -e/--extract pattern is required");
    }

    let rules: Vec<ExtractRule> = expressions
        .iter()
        .map(|expr| {
            let pattern = Regex::new(&expr.pattern)
                .with_context(|| format!("invalid regex: {}", &expr.pattern))?;
            let replacement = expr.replacement.clone();
            Ok(ExtractRule { pattern, replacement })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let stdin = io::stdin();
    let reader: Box<dyn BufRead> = match &app.file {
        Some(path) => {
            let file =
                File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
            Box::new(BufReader::new(file))
        }
        None => Box::new(stdin.lock()),
    };

    let strip = app.strip_ansi;
    let lines =
        reader.lines().map(move |l| l.map(|line| if strip { strip_ansi(&line) } else { line }));

    let sort = match app.sort {
        cli::SortStrategy::Appearance => SortStrategy::Appearance,
        cli::SortStrategy::Frequency => SortStrategy::Frequency,
        cli::SortStrategy::Alpha => SortStrategy::Alpha,
    };

    let results = extract(lines, &rules, sort, !app.no_dedup).context("failed to read input")?;

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    for item in &results {
        if app.line_number {
            writeln!(out, "{}:{}", item.line, item.text)?;
        } else {
            writeln!(out, "{}", item.text)?;
        }
    }

    Ok(())
}
