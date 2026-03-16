use std::io;
use std::ops::Range;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;

/// A single extraction rule: a regex pattern with an optional replacement template.
pub struct ExtractRule {
    pub pattern: Regex,
    pub replacement: Option<String>,
}

/// A matched item with its text and the line number where it first appeared.
#[derive(Debug, Clone)]
pub struct OutputItem {
    pub text: String,
    pub line: usize,
    pub count: usize,
}

/// Sort strategy for output items.
#[derive(Copy, Clone)]
pub enum SortStrategy {
    Appearance,
    Frequency,
    Alpha,
}

/// Regex to unwrap tmux DCS passthrough, keeping inner content.
static DCS_TMUX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1BPtmux;(.*?)\x1B\\").expect("invalid DCS tmux regex"));

/// Compiled regex for stripping ANSI escape sequences.
static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?:\x1B\[|\x9B)[0-9;?]*[ -/]*[@-~]", // CSI sequences (including 8-bit C1)
        r"|\x1B\][^\x07\x1B]*(?:\x07|\x1B\\)", // OSC sequences (hyperlinks, etc.)
        r"|\x1BP.*?\x1B\\",                    // DCS sequences (non-tmux, strip entirely)
        r"|\x1B[ -~]",                         // Fe/Fp/Fs escape sequences (2-char)
    ))
    .expect("invalid ANSI regex")
});

/// Strip ANSI escape sequences from input text.
///
/// Handles tmux DCS passthrough by unwrapping the envelope and un-doubling
/// ESC bytes before stripping the inner ANSI sequences.
pub fn strip_ansi(input: &str) -> String {
    // Step 1: Unwrap tmux DCS passthrough, keeping inner content
    let s = DCS_TMUX_RE.replace_all(input, "$1");
    // Step 2: Un-double ESC bytes from tmux passthrough encoding
    let s = s.replace("\x1B\x1B", "\x1B");
    // Step 3: Strip all remaining ANSI sequences
    ANSI_RE.replace_all(&s, "").into_owned()
}

/// Check if a range overlaps with any range in the consumed set.
fn overlaps(consumed: &[Range<usize>], range: &Range<usize>) -> bool {
    consumed.iter().any(|c| c.start < range.end && range.start < c.end)
}

/// Core extraction engine (streaming).
///
/// Consumes lines one at a time from the iterator, applying `rules` in priority order.
/// Each rule's matches are checked against already-consumed byte ranges on the same line;
/// overlapping matches are skipped. Only the extracted items are accumulated in memory.
///
/// Returns a list of `OutputItem`s, deduplicated and sorted according to the given strategy.
pub fn extract(
    lines: impl IntoIterator<Item = io::Result<String>>,
    rules: &[ExtractRule],
    sort: SortStrategy,
    dedup: bool,
) -> io::Result<Vec<OutputItem>> {
    let mut items: IndexMap<String, OutputItem> = IndexMap::new();

    for (line_idx, line_result) in lines.into_iter().enumerate() {
        let line = line_result?;
        let line_number = line_idx + 1;
        let mut consumed: Vec<Range<usize>> = Vec::new();

        for rule in rules {
            for caps in rule.pattern.captures_iter(&line) {
                let whole = caps.get(0).expect("capture group 0 always exists");
                let range = whole.start()..whole.end();

                if overlaps(&consumed, &range) {
                    continue;
                }
                consumed.push(range);

                let text = match &rule.replacement {
                    Some(template) => {
                        let mut dst = String::new();
                        caps.expand(template, &mut dst);
                        dst
                    }
                    None => whole.as_str().to_string(),
                };

                if dedup {
                    items
                        .entry(text.clone())
                        .and_modify(|item| item.count += 1)
                        .or_insert(OutputItem { text, line: line_number, count: 1 });
                } else {
                    let key = format!("{}\x00{}\x00{}", items.len(), line_number, &text);
                    items.insert(key, OutputItem { text, line: line_number, count: 1 });
                }
            }
        }
    }

    let mut results: Vec<OutputItem> = items.into_values().collect();

    match sort {
        SortStrategy::Appearance => {
            // IndexMap preserves insertion order, already correct
        }
        SortStrategy::Frequency => {
            results.sort_by_key(|item| std::cmp::Reverse(item.count));
        }
        SortStrategy::Alpha => {
            results.sort_by(|a, b| a.text.cmp(&b.text));
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(pattern: &str, replacement: Option<&str>) -> ExtractRule {
        ExtractRule {
            pattern: Regex::new(pattern).unwrap(),
            replacement: replacement.map(String::from),
        }
    }

    fn ok_lines(input: &str) -> impl Iterator<Item = io::Result<String>> + '_ {
        input.lines().map(|l| Ok(l.to_string()))
    }

    #[test]
    fn single_pattern_extract() {
        let rules = vec![rule(r"https?://\S+", None)];
        let results = extract(
            ok_lines("visit https://example.com today"),
            &rules,
            SortStrategy::Appearance,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "https://example.com");
        assert_eq!(results[0].line, 1);
    }

    #[test]
    fn extract_with_replacement() {
        let rules = vec![rule(r"git@([^:]+):(.+)\.git", Some("https://$1/$2"))];
        let results = extract(
            ok_lines("git@github.com:wfxr/xre.git"),
            &rules,
            SortStrategy::Appearance,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "https://github.com/wfxr/xre");
    }

    #[test]
    fn pattern_priority_consumes_range() {
        let rules = vec![rule(r"https?://\S+", None), rule(r"(www\.\S+)", Some("http://$1"))];
        let results = extract(
            ok_lines("visit https://www.example.com"),
            &rules,
            SortStrategy::Appearance,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "https://www.example.com");
    }

    #[test]
    fn dedup_keeps_first_occurrence() {
        let input = "https://example.com\nhttps://other.com\nhttps://example.com";
        let rules = vec![rule(r"https?://\S+", None)];
        let results = extract(ok_lines(input), &rules, SortStrategy::Appearance, true).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "https://example.com");
        assert_eq!(results[0].line, 1);
        assert_eq!(results[0].count, 2);
        assert_eq!(results[1].text, "https://other.com");
    }

    #[test]
    fn no_dedup_keeps_all() {
        let input = "https://example.com\nhttps://example.com";
        let rules = vec![rule(r"https?://\S+", None)];
        let results = extract(ok_lines(input), &rules, SortStrategy::Appearance, false).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "https://example.com");
        assert_eq!(results[1].text, "https://example.com");
    }

    #[test]
    fn sort_frequency() {
        let rules = vec![rule(r"[a-z]", None)];
        let results =
            extract(ok_lines("c\na\nb\na\nc\nc"), &rules, SortStrategy::Frequency, true).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].text, "c");
        assert_eq!(results[0].count, 3);
        assert_eq!(results[1].text, "a");
        assert_eq!(results[1].count, 2);
        assert_eq!(results[2].text, "b");
        assert_eq!(results[2].count, 1);
    }

    #[test]
    fn sort_alpha() {
        let rules = vec![rule(r"\w+", None)];
        let results =
            extract(ok_lines("cherry\napple\nbanana"), &rules, SortStrategy::Alpha, true).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].text, "apple");
        assert_eq!(results[1].text, "banana");
        assert_eq!(results[2].text, "cherry");
    }

    #[test]
    fn line_number_tracking() {
        let input = "aaa https://a.com bbb\nccc https://b.com ddd";
        let rules = vec![rule(r"https?://\S+", None)];
        let results = extract(ok_lines(input), &rules, SortStrategy::Appearance, true).unwrap();
        assert_eq!(results[0].line, 1);
        assert_eq!(results[0].text, "https://a.com");
        assert_eq!(results[1].line, 2);
        assert_eq!(results[1].text, "https://b.com");
    }

    #[test]
    fn empty_input() {
        let rules = vec![rule(r"https?://\S+", None)];
        let results = extract(ok_lines(""), &rules, SortStrategy::Appearance, true).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn no_match() {
        let rules = vec![rule(r"https?://\S+", None)];
        let results =
            extract(ok_lines("nothing here"), &rules, SortStrategy::Appearance, true).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_strip_ansi_sgr() {
        assert_eq!(strip_ansi("\x1b[31mhello\x1b[0m"), "hello");
        assert_eq!(strip_ansi("\x1b[1mbold\x1b[0m"), "bold");
        assert_eq!(strip_ansi("\x1b[38;5;196mred\x1b[0m"), "red");
    }

    #[test]
    fn test_strip_ansi_line_clear() {
        assert_eq!(strip_ansi("text\x1b[K"), "text");
        assert_eq!(strip_ansi("\x1b[2Jscreen"), "screen");
    }

    #[test]
    fn test_strip_ansi_cursor() {
        assert_eq!(strip_ansi("\x1b[10;20Hhere"), "here");
        assert_eq!(strip_ansi("\x1b[Aup"), "up");
    }

    #[test]
    fn test_strip_ansi_osc_hyperlink() {
        // OSC 8 hyperlink with BEL terminator
        assert_eq!(strip_ansi("\x1b]8;;https://example.com\x07link\x1b]8;;\x07"), "link");
        // OSC 8 hyperlink with ST terminator
        assert_eq!(strip_ansi("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"), "link");
    }

    #[test]
    fn test_strip_ansi_osc_title() {
        assert_eq!(strip_ansi("\x1b]0;window title\x07text"), "text");
    }

    #[test]
    fn test_strip_ansi_dcs_passthrough() {
        // tmux passthrough: ESC P tmux; <payload> ESC backslash
        assert_eq!(strip_ansi("\x1bPtmux;\x1b\x1b[32mhello\x1b[0m\x1b\\"), "hello");
        // Plain DCS sequence
        assert_eq!(strip_ansi("\x1bP0;1|1b5b316d\x1b\\text"), "text");
    }

    #[test]
    fn test_strip_ansi_fe_sequences() {
        // ESC = (keypad application mode), ESC > (keypad numeric mode)
        assert_eq!(strip_ansi("\x1b=hello\x1b>"), "hello");
        // ESC M (reverse index)
        assert_eq!(strip_ansi("\x1bMtext"), "text");
    }

    #[test]
    fn test_strip_ansi_plain_text() {
        assert_eq!(strip_ansi("no escapes here"), "no escapes here");
    }

    #[test]
    fn multiple_urls_one_line() {
        let rules = vec![rule(r"https?://\S+", None)];
        let results = extract(
            ok_lines("https://a.com and https://b.com"),
            &rules,
            SortStrategy::Appearance,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "https://a.com");
        assert_eq!(results[1].text, "https://b.com");
    }

    #[test]
    fn multiple_rules() {
        let input = "visit https://example.com and git@github.com:user/repo.git";
        let rules = vec![
            rule(r"https?://\S+", None),
            rule(r"git@([^:]+):(.+)\.git", Some("https://$1/$2")),
        ];
        let results = extract(ok_lines(input), &rules, SortStrategy::Appearance, true).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "https://example.com");
        assert_eq!(results[1].text, "https://github.com/user/repo");
    }

    #[test]
    fn real_url_patterns() {
        let rules = vec![rule(
            r"(https?|ftp|file):/?//[-A-Za-z0-9+&@#/%?=~_|!:,.;]*[-A-Za-z0-9+&@#/%=~_|]",
            None,
        )];
        let results = extract(
            ok_lines("check https://example.com/path?q=1&r=2#section end"),
            &rules,
            SortStrategy::Appearance,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "https://example.com/path?q=1&r=2#section");
    }

    #[test]
    fn trailing_punctuation_stripped_by_regex() {
        let rules = vec![rule(
            r"(https?|ftp|file):/?//[-A-Za-z0-9+&@#/%?=~_|!:,.;]*[-A-Za-z0-9+&@#/%=~_|]",
            None,
        )];
        let results =
            extract(ok_lines("Visit https://example.com."), &rules, SortStrategy::Appearance, true)
                .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "https://example.com");
    }

    #[test]
    fn ip_extraction_with_replacement() {
        let rules = vec![rule(
            r"[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}(:[0-9]{1,5})?(/\S+)*",
            Some("http://$0"),
        )];
        let results = extract(
            ok_lines("server at 10.0.0.1:8080/api"),
            &rules,
            SortStrategy::Appearance,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "http://10.0.0.1:8080/api");
    }

    #[test]
    fn git_ssh_url_replacement() {
        let rules = vec![rule(r"(ssh://)?git@([^/\s:]+)[:/](.+?)\.git", Some("https://$2/$3"))];
        let results = extract(
            ok_lines("ssh://git@github.com/user/repo.git"),
            &rules,
            SortStrategy::Appearance,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "https://github.com/user/repo");
    }

    #[test]
    fn github_shorthand_replacement() {
        let input = r#""user/repo" and 'org/project'"#;
        let rules = vec![rule(
            r#"['"]([_A-Za-z0-9-]*/[_.A-Za-z0-9-]*)['"]"#,
            Some("https://github.com/$1"),
        )];
        let results = extract(ok_lines(input), &rules, SortStrategy::Appearance, true).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "https://github.com/user/repo");
        assert_eq!(results[1].text, "https://github.com/org/project");
    }
}
