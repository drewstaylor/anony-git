# anony-git

A lightweight `git` wrapper that censors Personally Identifiable Information (PII) from git output before it reaches AI coding assistants.

## The Problem

AI tools like Claude Code and Cursor routinely read git history for context — commit logs, diffs, and more. Commands like `git log` and `git show` expose author names and email addresses in their output, leaking PII to the model. This can feel intrusive, and creates friction for teams with strict data handling policies.

`anony-git` strips author information from `git` commands known to leak it.

## How It Works

When `anony-git` receives a command that exposes author data, it injects the `--oneline` flag (if available), which limits output to the commit hash and subject line — no author names, no email addresses. If `--oneline` is not available (e.g. `git blame`, `git shortlog`) the output from `git` gets parsed and sanitized. All other commands are proxied as-is, so existing workflows are not disrupted.

## Supported Commands

| Command | Status |
|---|---|
| `git log` | Redacted |
| `git show` | Redacted |
| `git blame` | Planned |
| `git shortlog` | Planned |

All other commands pass through to `git` without modification.

## Setup

### 1. Build from Source

```bash
git clone git@github.com:drewstaylor/anony-git.git
cd anony-git
cargo build --release
```

The binary will be located at `./target/release/anony-git`.

### 2. Configure Your AI Tool

#### Claude Code

In your Claude Code terminal session, alias `git` to the `anony-git` binary:

```bash
alias git="~/YOUR_SYSTEM_PATH/anony-git/target/release/anony-git"
```

_(Replace `YOUR_SYSTEM_PATH` with the path where you cloned the repo.)_

> **Note:** It is recommended to set this alias only within your AI tool's terminal session, and not to export it globally. This keeps the wrapper scoped to the AI's use of `git`, while your own terminal sessions continue to use the real `git` binary.

#### Cursor

Add the following to your Cursor `settings.json`:

```json
{
  "git.path": "/absolute/path/to/anony-git/target/release/anony-git"
}
```

Replace the path with the absolute path to the binary on your system.

**Settings file locations:**
- macOS: `~/Library/Application Support/Cursor/User/settings.json`
- Linux: `~/.config/Cursor/User/settings.json`
- Windows: `%APPDATA%\Cursor\User\settings.json`

For a project-specific override, add `git.path` to `.vscode/settings.json` in the project root instead.

> **Note:** Shell aliases (e.g. `alias git=...`) do not work for Cursor's built-in git features. The `git.path` setting is required. Unlike the Claude Code alias approach, this also affects Cursor's SCM panel, not just the integrated terminal — which means author information will be redacted from Cursor's source control UI as well.

Restart Cursor after saving the setting.

## Roadmap

- `git blame` — shows author per line; needs a different approach as `--oneline` is not supported
- `git shortlog` — groups commits by author; its core purpose is to display authors, so this will require a custom formatting strategy
