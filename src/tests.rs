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
// shortlog - should inject --group=format:%as and HEAD, strip --group/--format flags
#[case(&["shortlog"], &["shortlog", "--group=format:%as", "HEAD"])]
#[case(&["shortlog", "--group=author"], &["shortlog", "--group=format:%as", "HEAD"])]
#[case(&["shortlog", "--format=%an"], &["shortlog", "--group=format:%as", "HEAD"])]
#[case(&["shortlog", "-n"], &["shortlog", "--group=format:%as", "-n", "HEAD"])]
#[case(&["--no-pager", "shortlog"], &["--no-pager", "shortlog", "--group=format:%as", "HEAD"])]
#[case(&["shortlog", "main..feature"], &["shortlog", "--group=format:%as", "main..feature"])]
#[case(&["shortlog", "-h"], &["shortlog", "-h"])]
#[case(&["shortlog", "--help"], &["shortlog", "--help"])]
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
#[case("shortlog", false)]
fn test_needs_oneline_redaction(#[case] subcommand: &str, #[case] expected: bool) {
    assert_eq!(needs_oneline_redaction(subcommand), expected);
}

#[rstest]
#[case("blame", true)]
#[case("log", false)]
#[case("show", false)]
#[case("status", false)]
#[case("shortlog", false)]
fn test_needs_blame_redaction(#[case] subcommand: &str, #[case] expected: bool) {
    assert_eq!(needs_blame_redaction(subcommand), expected);
}

#[rstest]
#[case("shortlog", true)]
#[case("log", false)]
#[case("show", false)]
#[case("blame", false)]
#[case("status", false)]
fn test_needs_shortlog_redaction(#[case] subcommand: &str, #[case] expected: bool) {
    assert_eq!(needs_shortlog_redaction(subcommand), expected);
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
// Basic shortlog - inject --group=format:%as and HEAD (no revision present)
#[case(&["shortlog"], &["shortlog", "--group=format:%as", "HEAD"])]
// --group flag is stripped and replaced
#[case(&["shortlog", "--group=author"], &["shortlog", "--group=format:%as", "HEAD"])]
#[case(&["shortlog", "--group=committer"], &["shortlog", "--group=format:%as", "HEAD"])]
// --format flag is stripped
#[case(&["shortlog", "--format=%an"], &["shortlog", "--group=format:%as", "HEAD"])]
// Other user flags are preserved, HEAD still injected (no revision)
#[case(&["shortlog", "-n"], &["shortlog", "--group=format:%as", "-n", "HEAD"])]
#[case(&["shortlog", "--numbered"], &["shortlog", "--group=format:%as", "--numbered", "HEAD"])]
// Global flags before subcommand are preserved
#[case(&["--no-pager", "shortlog"], &["--no-pager", "shortlog", "--group=format:%as", "HEAD"])]
#[case(&["-C", "/path", "shortlog"], &["-C", "/path", "shortlog", "--group=format:%as", "HEAD"])]
// Blocked and non-blocked flags mixed
#[case(&["shortlog", "--group=author", "-n"], &["shortlog", "--group=format:%as", "-n", "HEAD"])]
// Revision already present - HEAD not injected
#[case(&["shortlog", "main..feature"], &["shortlog", "--group=format:%as", "main..feature"])]
#[case(&["shortlog", "HEAD~10"], &["shortlog", "--group=format:%as", "HEAD~10"])]
#[case(&["shortlog", "-n", "main"], &["shortlog", "--group=format:%as", "-n", "main"])]
fn test_process_shortlog_args(#[case] input: &[&str], #[case] expected: &[&str]) {
    let a = args(input);
    let pos = find_subcommand_position(&a).unwrap();
    let result = process_shortlog_args(a, pos);
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
