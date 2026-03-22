use std::env;
use std::process::{Command, exit};

/// Git global options that consume the next argument as a value.
/// These need to be skipped when searching for the subcommand.
const GIT_FLAGS_WITH_VALUES: [&str; 7] = [
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
];

/// Flags that override `-s` and `--no-show-email` in `git blame`, causing PII
/// to appear in output even when both redaction flags are present. These must
/// be stripped before proxying the command to git.
const BLAME_BLOCKED_FLAGS: [&str; 5] = [
    "-p",
    "--porcelain",
    "--line-porcelain",
    "-e",
    "--incremental",
];

// TODO: Future support for other commands that leak PII:
// - `git shortlog`: Groups commits by author. Its purpose is to show authors.
//   Would need `--format` customization or a different approach.

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let git_args = process_args(args);

    let output = Command::new("git").args(&git_args).output();

    match output {
        Ok(result) => {
            print!("{}", String::from_utf8_lossy(&result.stdout));
            eprint!("{}", String::from_utf8_lossy(&result.stderr));
            exit(result.status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Failed to execute git: {}", e);
            exit(1);
        }
    }
}

/// Process arguments and apply PII redaction for commands that need it.
fn process_args(args: Vec<String>) -> Vec<String> {
    let subcommand_pos = find_subcommand_position(&args);

    match subcommand_pos {
        Some(pos) => {
            let subcommand = &args[pos];
            if needs_oneline_redaction(subcommand) && !has_flag_conflict(&args) {
                inject_oneline_after(args, pos)
            } else if needs_blame_redaction(subcommand) && !has_flag_conflict(&args) {
                process_blame_args(args, pos)
            } else {
                args
            }
        }
        None => args,
    }
}

/// Find the position of the git subcommand in the arguments.
/// Skips global flags and their values to find the actual subcommand.
fn find_subcommand_position(args: &[String]) -> Option<usize> {
    let mut skip_next = false;

    for (i, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }

        // Check if this flag consumes the next argument
        if GIT_FLAGS_WITH_VALUES.contains(&arg.as_str()) {
            skip_next = true;
            continue;
        }

        // Skip flags (arguments starting with -)
        if arg.starts_with('-') {
            continue;
        }

        // First non-flag argument is the subcommand
        return Some(i);
    }

    None
}

/// Check if the subcommand needs --oneline injection for PII redaction.
fn needs_oneline_redaction(subcommand: &str) -> bool {
    matches!(subcommand, "log" | "show")
}

/// Check if the subcommand needs blame-specific PII redaction.
fn needs_blame_redaction(subcommand: &str) -> bool {
    subcommand == "blame"
}

/// Check if --oneline flag, or help flags, are already present in the arguments.
fn has_flag_conflict(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--oneline" || arg == "-h" || arg == "--help")
}

/// Process `git blame` arguments: strip blocked flags that would leak PII even
/// when `-s` and `--no-show-email` are present, then inject both redaction
/// flags immediately after the subcommand.
fn process_blame_args(args: Vec<String>, _pos: usize) -> Vec<String> {
    let filtered: Vec<String> = args
        .into_iter()
        .filter(|arg| {
            !BLAME_BLOCKED_FLAGS.contains(&arg.as_str()) && arg != "-s" && arg != "--no-show-email"
        })
        .collect();

    let blame_pos = find_subcommand_position(&filtered).unwrap_or(0);

    let mut result = Vec::with_capacity(filtered.len() + 2);
    result.extend(filtered[..=blame_pos].iter().cloned());
    result.push("-s".to_string());
    result.push("--no-show-email".to_string());
    result.extend(filtered[blame_pos + 1..].iter().cloned());
    result
}

/// Insert --oneline flag immediately after the subcommand position.
fn inject_oneline_after(args: Vec<String>, pos: usize) -> Vec<String> {
    let mut result = Vec::with_capacity(args.len() + 1);
    result.extend(args[..=pos].iter().cloned());
    result.push("--oneline".to_string());
    result.extend(args[pos + 1..].iter().cloned());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Helper to convert &str slice to Vec<String>
    fn args(input: &[&str]) -> Vec<String> {
        input.iter().map(|s| s.to_string()).collect()
    }

    #[rstest]
    // Basic log/show commands - should inject --oneline
    #[case(&["log"], &["log", "--oneline"])]
    #[case(&["show"], &["show", "--oneline"])]
    #[case(&["log", "--all"], &["log", "--oneline", "--all"])]
    #[case(&["show", "abc123"], &["show", "--oneline", "abc123"])]
    #[case(&["log", "-n", "5"], &["log", "--oneline", "-n", "5"])]
    // Global flags before subcommand
    #[case(&["--no-pager", "log"], &["--no-pager", "log", "--oneline"])]
    #[case(&["-C", "/some/path", "log"], &["-C", "/some/path", "log", "--oneline"])]
    #[case(&["-c", "user.name=test", "show", "HEAD"], &["-c", "user.name=test", "show", "--oneline", "HEAD"])]
    #[case(&["--git-dir", "/path/.git", "log", "--all"], &["--git-dir", "/path/.git", "log", "--oneline", "--all"])]
    // Already has --oneline - should not duplicate
    #[case(&["log", "--oneline"], &["log", "--oneline"])]
    #[case(&["show", "--oneline", "abc123"], &["show", "--oneline", "abc123"])]
    #[case(&["--no-pager", "log", "--oneline"], &["--no-pager", "log", "--oneline"])]
    // blame - should inject -s and --no-show-email, strip blocked flags
    #[case(&["blame", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "-p", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "--porcelain", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "--line-porcelain", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "-e", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "--incremental", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "-s", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "--no-show-email", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "-s", "--no-show-email", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["--no-pager", "blame", "file.rs"], &["--no-pager", "blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "-h"], &["blame", "-h"])]
    #[case(&["blame", "--help"], &["blame", "--help"])]
    // Non-redacted commands - should pass through unchanged
    #[case(&["status"], &["status"])]
    #[case(&["diff"], &["diff"])]
    #[case(&["diff", "main...feature"], &["diff", "main...feature"])]
    #[case(&["commit", "-m", "message"], &["commit", "-m", "message"])]
    #[case(&["push", "origin", "main"], &["push", "origin", "main"])]
    #[case(&["checkout", "-b", "new-branch"], &["checkout", "-b", "new-branch"])]
    #[case(&["-C", "/path", "status"], &["-C", "/path", "status"])]
    // Edge cases
    #[case(&[], &[])] // No arguments
    #[case(&["--help"], &["--help"])] // Just a flag, no subcommand
    #[case(&["--version"], &["--version"])]
    fn test_process_args(#[case] input: &[&str], #[case] expected: &[&str]) {
        let result = process_args(args(input));
        assert_eq!(result, args(expected));
    }

    #[rstest]
    #[case(&["log"], Some(0))]
    #[case(&["show"], Some(0))]
    #[case(&["--no-pager", "log"], Some(1))]
    #[case(&["-C", "/path", "log"], Some(2))]
    #[case(&["-C", "/path", "--no-pager", "log"], Some(3))]
    #[case(&["--git-dir", "/path/.git", "-c", "key=val", "show"], Some(4))]
    #[case(&[], None)]
    #[case(&["--help"], None)]
    #[case(&["-C", "/path"], None)] // Flag with value but no subcommand
    fn test_find_subcommand_position(#[case] input: &[&str], #[case] expected: Option<usize>) {
        let result = find_subcommand_position(&args(input));
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case("log", true)]
    #[case("show", true)]
    #[case("status", false)]
    #[case("diff", false)]
    #[case("commit", false)]
    #[case("blame", false)]
    #[case("shortlog", false)] // TODO: Future support
    fn test_needs_oneline_redaction(#[case] subcommand: &str, #[case] expected: bool) {
        assert_eq!(needs_oneline_redaction(subcommand), expected);
    }

    #[rstest]
    #[case("blame", true)]
    #[case("log", false)]
    #[case("show", false)]
    #[case("status", false)]
    #[case("shortlog", false)] // TODO: Future support
    fn test_needs_blame_redaction(#[case] subcommand: &str, #[case] expected: bool) {
        assert_eq!(needs_blame_redaction(subcommand), expected);
    }

    #[rstest]
    // Both flags missing - inject both
    #[case(&["blame", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    // Only -s present - normalize to canonical position
    #[case(&["blame", "-s", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    // Only --no-show-email present - normalize to canonical position
    #[case(&["blame", "--no-show-email", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    // Both already present - normalize to canonical position
    #[case(&["blame", "-s", "--no-show-email", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    // Blocked flags are stripped
    #[case(&["blame", "-p", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "--porcelain", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "--line-porcelain", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "-e", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["blame", "--incremental", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    // Multiple blocked flags stripped at once
    #[case(&["blame", "-p", "-e", "file.rs"], &["blame", "-s", "--no-show-email", "file.rs"])]
    // Global flags before subcommand
    #[case(&["--no-pager", "blame", "file.rs"], &["--no-pager", "blame", "-s", "--no-show-email", "file.rs"])]
    #[case(&["-C", "/path", "blame", "file.rs"], &["-C", "/path", "blame", "-s", "--no-show-email", "file.rs"])]
    fn test_process_blame_args(#[case] input: &[&str], #[case] expected: &[&str]) {
        let a = args(input);
        let pos = find_subcommand_position(&a).unwrap();
        let result = process_blame_args(a, pos);
        assert_eq!(result, args(expected));
    }

    #[rstest]
    #[case(&["log", "--oneline"], true)]
    #[case(&["--oneline"], true)]
    #[case(&["log", "--all", "--oneline"], true)]
    #[case(&["log"], false)]
    #[case(&["log", "--all"], false)]
    #[case(&[], false)]
    fn test_has_flag_conflict(#[case] input: &[&str], #[case] expected: bool) {
        assert_eq!(has_flag_conflict(&args(input)), expected);
    }
}
