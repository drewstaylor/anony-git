#[cfg(test)]
mod tests;

use std::env;
use std::path::PathBuf;
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

/// Flag prefixes that control grouping and formatting in `git shortlog`.
/// These are stripped before injecting our canonical `--group=format:%as`,
/// as they could otherwise reintroduce author, email, or display name into output.
const SHORTLOG_BLOCKED_FLAG_PREFIXES: [&str; 2] = ["--group", "--format"];

/// Find the real git binary by walking PATH, skipping any entry that resolves
/// to the current executable (to avoid calling ourselves recursively when
/// installed as a `git` symlink on PATH).
fn find_real_git() -> Option<PathBuf> {
    let current_exe = env::current_exe().ok()?.canonicalize().ok()?;
    let path_var = env::var("PATH").ok()?;

    for dir in env::split_paths(&path_var) {
        let candidate = dir.join("git");
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical != current_exe {
                return Some(candidate);
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let git_args = process_args(args);

    let git_path = match find_real_git() {
        Some(path) => path,
        None => {
            eprintln!("anony-git: could not find git on PATH");
            exit(1);
        }
    };
    let output = Command::new(git_path).args(&git_args).output();

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
            } else if needs_shortlog_redaction(subcommand) && !has_flag_conflict(&args) {
                process_shortlog_args(args, pos)
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

/// Check if the subcommand needs shortlog-specific PII redaction.
fn needs_shortlog_redaction(subcommand: &str) -> bool {
    subcommand == "shortlog"
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

/// Process `git shortlog` arguments: strip any `--group` or `--format` flags
/// that could reintroduce PII into the grouping, inject `--group=format:%as`
/// immediately after the subcommand to group by date, and append `HEAD` if no
/// revision is present (required when using `--group=format:` to avoid reading
/// from stdin, which is unsupported with that flag).
fn process_shortlog_args(args: Vec<String>, _pos: usize) -> Vec<String> {
    let filtered: Vec<String> = args
        .into_iter()
        .filter(|arg| {
            !SHORTLOG_BLOCKED_FLAG_PREFIXES
                .iter()
                .any(|prefix| arg.starts_with(prefix))
        })
        .collect();

    let shortlog_pos = find_subcommand_position(&filtered).unwrap_or(0);

    let has_revision = filtered[shortlog_pos + 1..]
        .iter()
        .any(|arg| !arg.starts_with('-'));

    let mut result = Vec::with_capacity(filtered.len() + 2);
    result.extend(filtered[..=shortlog_pos].iter().cloned());
    result.push("--group=format:%as".to_string());
    result.extend(filtered[shortlog_pos + 1..].iter().cloned());
    if !has_revision {
        result.push("HEAD".to_string());
    }
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
