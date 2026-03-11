use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::db;

pub fn run(session_id: i64, running: Arc<AtomicBool>) {
    let mut repo_states: HashMap<String, String> = HashMap::new();

    // initial snapshot
    for repo in find_work_repos() {
        if let Some(status) = git_status_hash(&repo) {
            repo_states.insert(repo.to_string_lossy().to_string(), status);
        }
    }

    while running.load(Ordering::SeqCst) {
        for repo in find_work_repos() {
            let repo_str = repo.to_string_lossy().to_string();
            if let Some(current) = git_status_hash(&repo) {
                let changed = repo_states.get(&repo_str).map_or(true, |prev| prev != &current);
                if changed {
                    if let Some(diff) = git_diff_stat(&repo) {
                        let data = serde_json::json!({
                            "repo": repo.file_name().unwrap_or_default().to_string_lossy(),
                            "path": repo_str,
                            "insertions": diff.insertions,
                            "deletions": diff.deletions,
                            "files": diff.files,
                        });
                        db::save_event(session_id, "file_change", &data.to_string());
                    }
                    repo_states.insert(repo_str, current);
                }
            }
        }

        std::thread::sleep(Duration::from_secs(10));
    }
}

fn find_work_repos() -> Vec<PathBuf> {
    let work = dirs::home_dir().unwrap().join("work");
    if !work.exists() {
        return Vec::new();
    }

    let result = Command::new("find")
        .args([
            work.to_str().unwrap(),
            "-maxdepth",
            "3",
            "-name",
            ".git",
            "-type",
            "d",
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            text.lines()
                .filter(|l| !l.is_empty())
                .take(20)
                .map(|l| PathBuf::from(l).parent().unwrap().to_path_buf())
                .collect()
        }
        _ => Vec::new(),
    }
}

fn git_status_hash(repo: &PathBuf) -> Option<String> {
    Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

struct DiffStat {
    files: Vec<String>,
    insertions: u32,
    deletions: u32,
}

fn git_diff_stat(repo: &PathBuf) -> Option<DiffStat> {
    let output = Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(repo)
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return None;
    }

    let mut files = Vec::new();
    let mut insertions: u32 = 0;
    let mut deletions: u32 = 0;

    for line in text.lines() {
        if line.contains('|') {
            if let Some(fname) = line.split('|').next() {
                files.push(fname.trim().to_string());
            }
        }
        if line.contains("insertion") || line.contains("deletion") {
            for part in line.split(',') {
                let p = part.trim();
                if p.contains("insertion") {
                    if let Some(n) = p.split_whitespace().next().and_then(|s| s.parse().ok()) {
                        insertions = n;
                    }
                }
                if p.contains("deletion") {
                    if let Some(n) = p.split_whitespace().next().and_then(|s| s.parse().ok()) {
                        deletions = n;
                    }
                }
            }
        }
    }

    Some(DiffStat {
        files: files.into_iter().take(10).collect(),
        insertions,
        deletions,
    })
}
