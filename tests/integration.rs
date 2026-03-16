use assert_cmd::Command;
use predicates::prelude::*;

fn xre() -> Command {
    Command::cargo_bin("xre").unwrap()
}

#[test]
fn basic_stdin_extract() {
    xre()
        .args(["-e", r"https?://\S+"])
        .write_stdin("visit https://example.com today")
        .assert()
        .success()
        .stdout("https://example.com\n");
}

#[test]
fn extract_with_replacement() {
    xre()
        .args(["-e", r"git@([^:]+):(.+)\.git", "-r", "https://$1/$2"])
        .write_stdin("git@github.com:wfxr/xre.git")
        .assert()
        .success()
        .stdout("https://github.com/wfxr/xre\n");
}

#[test]
fn multiple_patterns_with_priority() {
    xre()
        .args(["-e", r"https?://\S+", "-e", r"(www\.\S+)", "-r", "http://$1"])
        .write_stdin("visit https://www.example.com")
        .assert()
        .success()
        .stdout("https://www.example.com\n");
}

#[test]
fn line_number_output() {
    xre()
        .args(["-n", "-e", r"https?://\S+"])
        .write_stdin("aaa https://a.com bbb\nccc https://b.com ddd")
        .assert()
        .success()
        .stdout("1:https://a.com\n2:https://b.com\n");
}

#[test]
fn no_patterns_errors() {
    xre()
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least one -e/--extract pattern is required"));
}

#[test]
fn file_input() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("input.txt");
    std::fs::write(&file_path, "hello https://test.com world\n").unwrap();

    xre()
        .args(["-e", r"https?://\S+"])
        .arg(&file_path)
        .assert()
        .success()
        .stdout("https://test.com\n");
}

#[test]
fn dedup_by_default() {
    xre()
        .args(["-e", r"https?://\S+"])
        .write_stdin("https://example.com\nhttps://example.com\nhttps://example.com")
        .assert()
        .success()
        .stdout("https://example.com\n");
}

#[test]
fn no_dedup_flag() {
    xre()
        .args(["--no-dedup", "-e", r"https?://\S+"])
        .write_stdin("https://example.com\nhttps://example.com")
        .assert()
        .success()
        .stdout("https://example.com\nhttps://example.com\n");
}

#[test]
fn sort_frequency() {
    xre()
        .args(["-e", r"[a-z]", "-s", "frequency"])
        .write_stdin("c\na\nb\na\nc\nc")
        .assert()
        .success()
        .stdout("c\na\nb\n");
}

#[test]
fn sort_alpha() {
    xre()
        .args(["-e", r"\w+", "-s", "alpha"])
        .write_stdin("cherry\napple\nbanana")
        .assert()
        .success()
        .stdout("apple\nbanana\ncherry\n");
}

#[test]
fn strip_ansi_flag() {
    xre()
        .args(["--strip-ansi", "-e", r"https?://\S+"])
        .write_stdin("\x1b[32mhttps://example.com\x1b[0m")
        .assert()
        .success()
        .stdout("https://example.com\n");
}

#[test]
fn empty_input_no_output() {
    xre().args(["-e", r"https?://\S+"]).write_stdin("").assert().success().stdout("");
}

#[test]
fn invalid_regex_errors() {
    xre()
        .args(["-e", r"[invalid"])
        .write_stdin("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid regex"));
}

#[test]
fn replace_before_extract_errors() {
    xre()
        .args(["-r", "replacement", "-e", "pattern"])
        .write_stdin("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("-r/--replace must follow a -e/--extract"));
}

#[test]
fn duplicate_replace_errors() {
    xre()
        .args(["-e", "pattern", "-r", "repl1", "-r", "repl2"])
        .write_stdin("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate -r/--replace"));
}

#[test]
fn tmux_fzf_url_real_scenario() {
    let input = "\
visit https://github.com/wfxr/xre for source\n\
also www.example.com/docs has info\n\
clone with git@github.com:wfxr/tmux-fzf-url.git\n\
server at 192.168.1.100:8080/api\n\
check 'my-org/my-repo' on github\n\
https://github.com/wfxr/xre again\n";

    xre()
        .args([
            "-e",
            r"(https?|ftp|file):/?//[-A-Za-z0-9+&@#/%?=~_|!:,.;]*[-A-Za-z0-9+&@#/%=~_|]",
            "-e",
            r"git@([^:]+):(.+)\.git",
            "-r",
            "https://$1/$2",
            "-e",
            r"(www\.\S+)",
            "-r",
            "http://$1",
        ])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(
            "https://github.com/wfxr/xre\n\
             http://www.example.com/docs\n\
             https://github.com/wfxr/tmux-fzf-url\n",
        );
}

#[test]
fn strip_ansi_dcs_passthrough() {
    xre()
        .args(["--strip-ansi", "-e", r"https?://\S+"])
        .write_stdin("\x1bPtmux;\x1b\x1b[32mhttps://example.com\x1b[0m\x1b\\")
        .assert()
        .success()
        .stdout("https://example.com\n");
}
