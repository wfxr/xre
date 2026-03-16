<p align="center">
  <h1 align="center">xre</h1>
  <p align="center">
    A fast regex extraction tool with pattern matching, replacement, and configurable sorting.
  </p>
</p>

## Features

- **Multi-pattern extraction** — apply multiple regex patterns in a single pass with priority ordering
- **Match + replace** — optional replacement templates with capture group substitution (`$1`, `$2`, ...)
- **Deduplication** — automatic dedup with first-occurrence tracking (disable with `--no-dedup`)
- **Sorting** — sort by appearance (default), frequency, or alphabetically
- **Line numbers** — optional line number prefix (`-n`)
- **ANSI stripping** — strip terminal escape sequences before processing (`--strip-ansi`)
- **Line-oriented** — input is processed line by line; each pattern matches within a single line

## Installation

### From crates.io

```bash
cargo install xre
```

### From source

```bash
cargo install --path .
```

### From binaries

Download pre-built binaries from the [releases page](https://github.com/wfxr/xre/releases).

## Usage

```
xre [OPTIONS] [FILE]

Options:
  -e, --extract <PATTERN>                Extract pattern (repeatable, earlier = higher priority)
  -r, --replace <REPLACEMENT>            Replacement for the preceding -e ($1, $2, ...)
  -s, --sort <STRATEGY>                  Sort strategy: appearance (default), frequency, alpha
      --no-dedup                         Disable deduplication
  -n, --line-number                      Prefix each match with line number (LINE:MATCH)
      --strip-ansi                       Strip ANSI escape sequences before processing
  -h, --help                             Print help
  -V, --version                          Print version
```

### Examples

**Basic URL extraction:**
```bash
echo "visit https://example.com today" | xre -e 'https?://\S+'
# Output: https://example.com
```

**Extract + replace (git SSH → HTTPS):**
```bash
echo "git@github.com:wfxr/xre.git" | xre -e 'git@([^:]+):(.+)\.git' -r 'https://$1/$2'
# Output: https://github.com/wfxr/xre
```

**Multi-pattern with priority:**
```bash
echo "visit https://www.example.com" | xre \
  -e 'https?://\S+' \
  -e '(www\.\S+)' -r 'http://$1'
# Output: https://www.example.com (second pattern skipped — range already consumed)
```

**tmux-fzf-url integration:**
```bash
tmux capture-pane -J -p -e | xre --strip-ansi \
  -e '(https?|ftp|file):/?//[-A-Za-z0-9+&@#/%?=~_|!:,.;]*[-A-Za-z0-9+&@#/%=~_|]' \
  -e 'git@([^:]+):(.+)\.git' -r 'https://$1/$2' \
  -e '(www\.\S+)' -r 'http://$1' \
  -s appearance
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
