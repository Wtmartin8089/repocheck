use clap::Parser;
use colored::*;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

// ── CLI ────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "repocheck",
    version,
    about = "Git repository health checker",
    long_about = "Scans a git repo for common problems: stale branches, missing files, hardcoded secrets, outdated patterns, and more."
)]
struct Args {
    /// Path to the repository (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Show only failures and warnings (suppress passes)
    #[arg(long)]
    failures_only: bool,
}

// ── Check Result ───────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
enum Status {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, serde::Serialize)]
struct CheckResult {
    name: String,
    status: Status,
    message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    details: Vec<String>,
}

impl CheckResult {
    fn pass(name: &str, msg: &str) -> Self {
        Self { name: name.into(), status: Status::Pass, message: msg.into(), details: vec![] }
    }
    fn warn(name: &str, msg: &str, details: Vec<String>) -> Self {
        Self { name: name.into(), status: Status::Warn, message: msg.into(), details }
    }
    fn fail(name: &str, msg: &str, details: Vec<String>) -> Self {
        Self { name: name.into(), status: Status::Fail, message: msg.into(), details }
    }
}

// ── Checks ─────────────────────────────────────────────────────────────────────

fn check_is_git_repo(path: &Path) -> CheckResult {
    if path.join(".git").exists() {
        CheckResult::pass("git-repo", "Is a git repository")
    } else {
        CheckResult::fail("git-repo", "Not a git repository", vec![])
    }
}

fn check_required_files(path: &Path) -> CheckResult {
    let required = [".gitignore", "README.md", "LICENSE"];
    let missing: Vec<String> = required
        .iter()
        .filter(|f| !path.join(f).exists())
        .map(|f| f.to_string())
        .collect();

    if missing.is_empty() {
        CheckResult::pass("required-files", "README.md, LICENSE, .gitignore all present")
    } else {
        CheckResult::warn(
            "required-files",
            &format!("{} required file(s) missing", missing.len()),
            missing,
        )
    }
}

fn check_stale_branches(path: &Path) -> CheckResult {
    let output = Command::new("git")
        .args(["branch", "--format=%(refname:short) %(committerdate:relative)"])
        .current_dir(path)
        .output();

    let Ok(out) = output else {
        return CheckResult::warn("stale-branches", "Could not read branches", vec![]);
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let stale: Vec<String> = text
        .lines()
        .filter(|l| {
            (l.contains("months ago") || l.contains("years ago"))
                && !l.starts_with("main")
                && !l.starts_with("master")
        })
        .map(|l| l.trim().to_string())
        .collect();

    if stale.is_empty() {
        CheckResult::pass("stale-branches", "No stale branches found")
    } else {
        CheckResult::warn(
            "stale-branches",
            &format!("{} potentially stale branch(es)", stale.len()),
            stale,
        )
    }
}

fn check_uncommitted_changes(path: &Path) -> CheckResult {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output();

    let Ok(out) = output else {
        return CheckResult::warn("uncommitted", "Could not read git status", vec![]);
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let changes: Vec<String> = text.lines().map(|l| l.trim().to_string()).collect();

    if changes.is_empty() {
        CheckResult::pass("uncommitted", "Working tree is clean")
    } else {
        CheckResult::warn(
            "uncommitted",
            &format!("{} uncommitted change(s)", changes.len()),
            changes,
        )
    }
}

fn check_secrets(path: &Path) -> CheckResult {
    let patterns: Vec<(&str, &str)> = vec![
        (r#"(?i)(password|passwd|pwd)\s*=\s*['"][^'"]{4,}"#, "Hardcoded password"),
        (r#"(?i)(api_key|apikey|api-key)\s*=\s*['"][^'"]{8,}"#, "Hardcoded API key"),
        (r#"(?i)secret\s*=\s*['"][^'"]{8,}"#, "Hardcoded secret"),
        (r#"(?i)(token)\s*=\s*['"][^'"]{8,}"#, "Hardcoded token"),
        (r"AKIA[0-9A-Z]{16}", "AWS access key"),
        (r#"(?i)private.?key\s*=\s*['"][^'"]{8,}"#, "Private key value"),
    ];

    let compiled: Vec<(Regex, &str)> = patterns
        .into_iter()
        .filter_map(|(p, label)| Regex::new(p).ok().map(|r| (r, label)))
        .collect();

    let skip_dirs = [".git", "target", "node_modules", ".venv", "venv"];
    let skip_exts = ["png", "jpg", "jpeg", "gif", "svg", "ico", "woff", "ttf", "bin", "lock"];

    let mut hits: Vec<String> = vec![];

    'outer: for entry in WalkDir::new(path).into_iter().flatten() {
        let ep = entry.path();
        for part in ep.components() {
            if skip_dirs.iter().any(|d| part.as_os_str() == *d) {
                continue 'outer;
            }
        }
        if ep.is_dir() { continue; }
        if let Some(ext) = ep.extension().and_then(|e| e.to_str()) {
            if skip_exts.contains(&ext) { continue; }
        }
        let Ok(content) = std::fs::read_to_string(ep) else { continue };
        for (re, label) in &compiled {
            if re.is_match(&content) {
                let rel = ep.strip_prefix(path).unwrap_or(ep);
                hits.push(format!("{}: {}", rel.display(), label));
                break;
            }
        }
        if hits.len() >= 10 { break; }
    }

    if hits.is_empty() {
        CheckResult::pass("secrets", "No obvious hardcoded secrets found")
    } else {
        CheckResult::fail(
            "secrets",
            &format!("{} file(s) may contain hardcoded secrets", hits.len()),
            hits,
        )
    }
}

fn check_gitignore_coverage(path: &Path) -> CheckResult {
    let gitignore = path.join(".gitignore");
    let Ok(content) = std::fs::read_to_string(gitignore) else {
        return CheckResult::warn("gitignore", ".gitignore missing or unreadable", vec![]);
    };

    let expected = [".env", "target/", "node_modules/", "__pycache__/", ".venv/", "*.log"];
    let missing: Vec<String> = expected
        .iter()
        .filter(|e| !content.contains(*e))
        .map(|e| e.to_string())
        .collect();

    if missing.is_empty() {
        CheckResult::pass("gitignore", ".gitignore covers common patterns")
    } else {
        CheckResult::warn(
            "gitignore",
            &format!("{} common pattern(s) not in .gitignore", missing.len()),
            missing,
        )
    }
}

fn check_large_files(path: &Path) -> CheckResult {
    let threshold_mb: u64 = 10;
    let skip_dirs = [".git", "target", "node_modules"];

    let mut large: Vec<String> = vec![];

    'outer: for entry in WalkDir::new(path).into_iter().flatten() {
        let ep = entry.path();
        for part in ep.components() {
            if skip_dirs.iter().any(|d| part.as_os_str() == *d) {
                continue 'outer;
            }
        }
        if ep.is_file() {
            if let Ok(meta) = ep.metadata() {
                let mb = meta.len() / (1024 * 1024);
                if mb >= threshold_mb {
                    let rel = ep.strip_prefix(path).unwrap_or(ep);
                    large.push(format!("{} ({}MB)", rel.display(), mb));
                }
            }
        }
    }

    if large.is_empty() {
        CheckResult::pass("large-files", &format!("No files over {}MB", threshold_mb))
    } else {
        CheckResult::warn(
            "large-files",
            &format!("{} large file(s) found — consider git-lfs", large.len()),
            large,
        )
    }
}

fn check_last_commit(path: &Path) -> CheckResult {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%cr"])
        .current_dir(path)
        .output();

    let Ok(out) = output else {
        return CheckResult::warn("last-commit", "Could not read commit history", vec![]);
    };

    let age = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if age.is_empty() {
        return CheckResult::warn("last-commit", "No commits found", vec![]);
    }

    if age.contains("years") {
        CheckResult::warn(
            "last-commit",
            &format!("Last commit was {} — repo may be abandoned", age),
            vec![],
        )
    } else {
        CheckResult::pass("last-commit", &format!("Last commit: {}", age))
    }
}

// ── Output ─────────────────────────────────────────────────────────────────────

fn print_result(r: &CheckResult, failures_only: bool) {
    match r.status {
        Status::Pass => {
            if !failures_only {
                println!("  {}  {}", "✓".green().bold(), r.message);
            }
        }
        Status::Warn => {
            println!("  {}  {}", "⚠".yellow().bold(), r.message.yellow());
            for d in &r.details {
                println!("       {}", d.dimmed());
            }
        }
        Status::Fail => {
            println!("  {}  {}", "✗".red().bold(), r.message.red().bold());
            for d in &r.details {
                println!("       {}", d.dimmed());
            }
        }
    }
}

fn summary_line(results: &[CheckResult]) {
    let passes = results.iter().filter(|r| matches!(r.status, Status::Pass)).count();
    let warns  = results.iter().filter(|r| matches!(r.status, Status::Warn)).count();
    let fails  = results.iter().filter(|r| matches!(r.status, Status::Fail)).count();

    println!();
    println!(
        "  {}  {}  {}",
        format!("{} passed", passes).green().bold(),
        format!("{} warnings", warns).yellow().bold(),
        format!("{} failed", fails).red().bold(),
    );
}

// ── Main ───────────────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();
    let path = args.path.canonicalize().unwrap_or(args.path.clone());

    if !args.json {
        println!();
        println!("{}", format!("repocheck — {}", path.display()).bold());
        println!("{}", "─".repeat(50).dimmed());
    }

    let results = vec![
        check_is_git_repo(&path),
        check_required_files(&path),
        check_last_commit(&path),
        check_uncommitted_changes(&path),
        check_stale_branches(&path),
        check_gitignore_coverage(&path),
        check_secrets(&path),
        check_large_files(&path),
    ];

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
        return;
    }

    for r in &results {
        print_result(r, args.failures_only);
    }

    summary_line(&results);
    println!();

    if results.iter().any(|r| matches!(r.status, Status::Fail)) {
        std::process::exit(1);
    }
}
