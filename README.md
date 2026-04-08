# repocheck

🚀 RepoCheck Pro available

Get the full version with advanced checks, secret detection, and JSON output:
👉https://wayne-soft-hub.vercel.app/

A fast, zero-dependency CLI tool that scans any git repository for common health problems — before they become bigger issues.

```
repocheck — /home/wayne/myproject
──────────────────────────────────────────────────
  ✓  Is a git repository
  ✓  README.md, LICENSE, .gitignore all present
  ✓  Last commit: 2 hours ago
  ⚠  3 uncommitted change(s)
  ✓  No stale branches found
  ⚠  2 common pattern(s) not in .gitignore
       node_modules/
       .env
  ✗  2 file(s) may contain hardcoded secrets
       src/config.py: Hardcoded API key
       .env.example: Hardcoded token
  ✓  No files over 10MB

  5 passed  2 warnings  1 failed
```

## What It Checks

| Check | Description |
|---|---|
| **git-repo** | Confirms the path is a git repository |
| **required-files** | README.md, LICENSE, .gitignore present |
| **last-commit** | Flags repos with no recent activity |
| **uncommitted** | Detects uncommitted or untracked changes |
| **stale-branches** | Branches untouched for months/years |
| **gitignore** | Missing common patterns (.env, target/, node_modules/, etc.) |
| **secrets** | Hardcoded passwords, API keys, tokens, AWS credentials |
| **large-files** | Files over 10MB that should be in git-lfs |

## Install

### From Release (Linux x86_64)

```bash
curl -L https://github.com/Wtmartin8089/repocheck/releases/latest/download/repocheck -o ~/.local/bin/repocheck
chmod +x ~/.local/bin/repocheck
```

### From Source

```bash
git clone https://github.com/Wtmartin8089/repocheck
cd repocheck
cargo build --release
cp target/release/repocheck ~/.local/bin/
```

### Arch Linux (AUR)

```bash
yay -S repocheck
```

## Usage

```bash
# Check current directory
repocheck

# Check a specific repo
repocheck ~/projects/myapp

# Show only problems (suppress passing checks)
repocheck --failures-only

# JSON output (for scripting or CI)
repocheck --json
```

## CI Integration

```yaml
# GitHub Actions example
- name: Repo health check
  run: repocheck --failures-only --json
```

## License

MIT — see [LICENSE](LICENSE)
