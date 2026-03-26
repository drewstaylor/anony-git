# anony-git

A lightweight `git` wrapper that censors Personally Identifiable Information (PII) from git output before it reaches AI coding assistants.

## The Problem

AI tools like [Claude Code](https://code.claude.com) and [Cursor](https://cursor.com/) routinely read git history for context — commit logs, diffs, and more. Commands like `git log` and `git show` expose author names and email addresses in their output, leaking PII to the model. This can feel intrusive, and creates friction for teams with strict data handling policies.

`anony-git` strips author information from `git` commands known to leak it.

## How It Works

`anony-git` intercepts `git` commands and applies command-specific redaction before proxying to `git`. All other commands pass through unchanged.

- **`git log`, `git show`** — injects `--oneline`, limiting output to the commit hash and subject line with no author or email.
- **`git blame`** — strips flags that would expose author data (`-p`, `--porcelain`, `--line-porcelain`, `-e`, `--incremental`) and injects `-s` and `--no-show-email` to suppress the author name and email fields.
- **`git shortlog`** — strips any user-supplied `--group` or `--format` flags and injects `--group=format:%as`, grouping output by date instead of by author or email.

## Supported Commands

| Command | Status |
|---|---|
| `git log` | Redacted |
| `git show` | Redacted |
| `git blame` | Redacted |
| `git shortlog` | Redacted |

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

Shell aliases are not inherited by Claude Code's subprocesses, so a symlink on `PATH` is required instead.

**Step 1** — Create a `git` symlink pointing to the `anony-git` binary:

_macOS / Linux:_
```bash
mkdir -p ~/.claude/bin
ln -s /absolute/path/to/anony-git/target/release/anony-git ~/.claude/bin/git
```

_Windows (PowerShell, run as Administrator):_
```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\.claude\bin"
New-Item -ItemType SymbolicLink -Path "$env:USERPROFILE\.claude\bin\git.exe" -Target "C:\absolute\path\to\anony-git\target\release\anony-git.exe"
```

**Step 2** — Add the symlink directory to the front of `PATH` in Claude Code's settings.

> **Critical:** Claude Code does not support `$PATH` expansion in the `env` block — you must hardcode the full path string. Do **not** just paste a minimal example like `/usr/bin:/bin`. If you omit directories that your system tools live in (Homebrew, nvm, Cargo, pyenv, etc.), Claude Code will lose access to those tools entirely, breaking commands that rely on them.

The safe approach is to prepend `~/.claude/bin` to your **existing `PATH`**. First, print your current PATH in a terminal:

```bash
echo $PATH
```

Then, in your `settings.json`, paste that output as the value — with `~/.claude/bin:` (macOS/Linux) or `%USERPROFILE%\.claude\bin;` (Windows) added to the **front**:

_macOS / Linux_ (`~/.claude/settings.json`):
```json
{
  "env": {
    "PATH": "/Users/YOUR_USERNAME/.claude/bin:<paste your echo $PATH output here>"
  }
}
```

For example, a realistic entry might look like:
```json
{
  "env": {
    "PATH": "/Users/alice/.claude/bin:/opt/homebrew/bin:/Users/alice/.nvm/versions/node/v24.7.0/bin:/usr/local/bin:/usr/bin:/bin:/Users/alice/.cargo/bin"
  }
}
```

_Windows_ (`%USERPROFILE%\.claude\settings.json`):
```json
{
  "env": {
    "PATH": "C:\\Users\\YOUR_USERNAME\\.claude\\bin;<paste your $env:PATH output here>"
  }
}
```

> **Note:** To scope the configuration to a single project rather than all Claude Code sessions, add the same `env` block to `.claude/settings.json` in the project root instead of the global settings file.

**How it works without infinite recursion:** When `anony-git` (symlinked as `git`) is invoked, it detects its own location on `PATH` and skips it when searching for the real `git` binary — so placing `~/.claude/bin` first is safe and won't cause `anony-git` to call itself.

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

> **Note:** Shell aliases (e.g. `alias git=...`) do not work for Cursor's built-in git features. The `git.path` setting is required. This also affects Cursor's SCM panel, not just the integrated terminal — which means author information will be redacted from Cursor's source control UI as well.

Unlike the Claude Code setup, Cursor's `git.path` points directly to the `anony-git` binary — there is no `git` symlink involved, so there is no risk of infinite recursion. `anony-git` finds the real `git` by searching the `PATH` that Cursor inherits from your shell environment, which already contains the system `git`.

Restart Cursor after saving the setting.

