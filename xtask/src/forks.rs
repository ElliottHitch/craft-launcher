//! Local checkouts for the personal forks. Upstream changes are fetched, never merged.

use artcraft_launcher_core::catalog::{CraftApp, LAUNCHER, SUITE};
use std::path::{Path, PathBuf};
use std::process::Command;

const USER_AGENT: &str = concat!("http.userAgent=ArtCraft-Launcher/", env!("CARGO_PKG_VERSION"));

const USAGE: &str = "\
usage: cargo xtask forks <list|status|clone|fetch> [app]

  list    show the catalog's fork, upstream, and checkout paths
  status  show each checkout's branch, commit, and working-tree state
  clone   clone missing forks with full history and add the upstream remote
  fetch   fetch upstream branches into refs/remotes/upstream only

The launcher checkout is this workspace; app checkouts are in repos/<repo>.
Select an app by its catalog id or repository name, including craft-launcher.
Clone and fetch refuse shallow or mismatched existing checkouts.
No command merges, rebases, resets, pushes, or publishes releases.
";

struct Fork {
    app: &'static CraftApp,
    repo: &'static str,
    path: PathBuf,
}

impl Fork {
    fn new(app: &'static CraftApp) -> Result<Self, String> {
        let (_, repo) = app.repo.split_once('/').ok_or_else(|| format!("invalid catalog repository for {}", app.id))?;
        let path = if app.id == LAUNCHER.id { crate::root() } else { crate::root().join("repos").join(repo) };
        Ok(Self { app, repo, path })
    }

    fn origin(&self) -> String {
        format!("https://github.com/{}.git", self.app.repo)
    }

    fn upstream(&self) -> String {
        format!("https://github.com/storytold/{}.git", self.repo)
    }
}

pub fn run(args: &[&str]) -> Result<(), String> {
    let (command, rest) = args.split_first().ok_or_else(|| USAGE.to_string())?;
    if matches!(*command, "help" | "--help" | "-h") {
        print!("{USAGE}");
        return Ok(());
    }
    if !matches!(*command, "list" | "status" | "clone" | "fetch") || rest.len() > 1 {
        return Err(USAGE.into());
    }
    let all = std::iter::once(&LAUNCHER).chain(SUITE.iter()).map(Fork::new).collect::<Result<Vec<_>, _>>()?;
    let selected: Vec<_> = all.iter().filter(|fork| rest.first().is_none_or(|app| *app == fork.app.id || *app == fork.repo)).collect();
    if selected.is_empty() {
        return Err(format!("unknown app `{}`; use `cargo xtask forks list`", rest.first().copied().unwrap_or_default()));
    }
    let mut failed = 0;
    for fork in selected {
        let result = match *command {
            "list" => {
                println!("{}\n  origin:   {}\n  upstream: {}\n  checkout: {}", fork.app.id, fork.origin(), fork.upstream(), fork.path.display());
                Ok(())
            }
            "status" => status(fork),
            "clone" => clone(fork),
            "fetch" => fetch(fork),
            _ => return Err(USAGE.into()),
        };
        if let Err(error) = result {
            eprintln!("{}: {error}", fork.app.id);
            failed += 1;
        }
    }
    if failed == 0 { Ok(()) } else { Err(format!("{failed} checkout(s) need attention")) }
}

fn git(path: &Path, args: &[&str]) -> Result<String, String> {
    let output =
        Command::new("git").args(["-c", USER_AGENT]).arg("-C").arg(path).args(args).output().map_err(|e| format!("couldn't run git ({e})"))?;
    if !output.status.success() {
        return Err(format!("git {}: {}", args.first().copied().unwrap_or_default(), String::from_utf8_lossy(&output.stderr).trim()));
    }
    String::from_utf8(output.stdout).map(|s| s.trim().to_string()).map_err(|e| format!("git returned invalid text ({e})"))
}

fn matches_remote(url: &str, expected: &str) -> bool {
    let url = url.trim_end_matches('/').strip_suffix(".git").unwrap_or(url.trim_end_matches('/'));
    let expected = expected.trim_end_matches(".git");
    url.eq_ignore_ascii_case(expected)
        || url.eq_ignore_ascii_case(&expected.replacen("https://github.com/", "git@github.com:", 1))
        || url.eq_ignore_ascii_case(&expected.replacen("https://github.com/", "ssh://git@github.com/", 1))
}

/// Return whether upstream is present; an existing remote must match before any mutation.
fn inspect(fork: &Fork) -> Result<bool, String> {
    let top = git(&fork.path, &["rev-parse", "--show-toplevel"])?;
    let actual = std::fs::canonicalize(top).map_err(|e| format!("couldn't resolve checkout ({e})"))?;
    let expected = std::fs::canonicalize(&fork.path).map_err(|e| format!("couldn't resolve {} ({e})", fork.path.display()))?;
    if actual != expected {
        return Err(format!("{} is not a repository root", fork.path.display()));
    }
    let remotes = git(&fork.path, &["remote"])?;
    for (remote, expected_url) in [("origin", fork.origin()), ("upstream", fork.upstream())] {
        if !remotes.lines().any(|name| name == remote) {
            if remote == "origin" {
                return Err(format!("origin is missing; expected {expected_url}"));
            }
            continue;
        }
        for args in [["remote", "get-url", "--all", remote].as_slice(), ["remote", "get-url", "--push", "--all", remote].as_slice()] {
            let urls = git(&fork.path, args)?;
            if urls.is_empty() || !urls.lines().all(|url| matches_remote(url, &expected_url)) {
                return Err(format!("{remote} does not point exclusively at {expected_url}; leaving this checkout unchanged"));
            }
        }
    }
    Ok(remotes.lines().any(|name| name == "upstream"))
}

fn status(fork: &Fork) -> Result<(), String> {
    if !fork.path.exists() {
        println!("{}: missing ({})", fork.app.id, fork.path.display());
        return Ok(());
    }
    let upstream = inspect(fork)?;
    let branch = git(&fork.path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let commit = git(&fork.path, &["rev-parse", "--short", "HEAD"])?;
    let dirty = !git(&fork.path, &["status", "--porcelain"])?.is_empty();
    let shallow = git(&fork.path, &["rev-parse", "--is-shallow-repository"])?;
    println!(
        "{}: {branch} {commit}, {}, upstream {}, history {}",
        fork.app.id,
        if dirty { "dirty" } else { "clean" },
        if upstream { "configured" } else { "missing" },
        if shallow == "false" { "full" } else { "shallow" },
    );
    Ok(())
}

fn prepare(fork: &Fork) -> Result<(), String> {
    let upstream = inspect(fork)?;
    if git(&fork.path, &["rev-parse", "--is-shallow-repository"])? != "false" {
        return Err("checkout is shallow; fetch its full history before this command".into());
    }
    if !upstream {
        git(&fork.path, &["remote", "add", "upstream", &fork.upstream()])?;
    }
    Ok(())
}

fn clone(fork: &Fork) -> Result<(), String> {
    if !fork.path.exists() {
        let parent = fork.path.parent().ok_or("checkout path has no parent")?;
        std::fs::create_dir_all(parent).map_err(|e| format!("couldn't create {} ({e})", parent.display()))?;
        eprintln!("cloning {} into {}", fork.app.repo, fork.path.display());
        let result = Command::new("git")
            .args(["-c", USER_AGENT])
            .args(["clone", "--filter=blob:none", "--", &fork.origin()])
            .arg(&fork.path)
            .status()
            .map_err(|e| format!("couldn't run git clone ({e})"))?;
        if !result.success() {
            return Err(format!("git clone exited with {result}; inspect {} before retrying", fork.path.display()));
        }
    }
    prepare(fork)?;
    println!("{}: ready ({})", fork.app.id, fork.path.display());
    Ok(())
}

fn fetch(fork: &Fork) -> Result<(), String> {
    if !fork.path.exists() {
        return Err(format!("checkout is missing; run `cargo xtask forks clone {}`", fork.app.id));
    }
    prepare(fork)?;
    // An explicit refspec prevents local remote configuration from updating checked-out branches.
    git(&fork.path, &["-c", "fetch.recurseSubmodules=false", "fetch", "--no-tags", "upstream", "+refs/heads/*:refs/remotes/upstream/*"])?;
    println!("{}: fetched upstream branches", fork.app.id);
    Ok(())
}
