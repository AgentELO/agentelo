use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::db;

const AI_COMMANDS: &[&str] = &[
    "claude", "aider", "cursor", "copilot", "sgpt", "llm ",
    "chatgpt", "openai", "anthropic",
];

const AI_PROCESSES: &[&str] = &["claude", "cursor", "copilot", "aider", "cody"];

pub fn run(session_id: i64, running: Arc<AtomicBool>) {
    let history_file = find_history_file();
    let mut last_line_count = history_file
        .as_ref()
        .map(|f| count_lines(f))
        .unwrap_or(0);

    while running.load(Ordering::SeqCst) {
        // check history for new commands
        if let Some(ref hf) = history_file {
            let current = count_lines(hf);
            if current > last_line_count {
                let new_lines = get_new_lines(hf, current - last_line_count);
                for line in new_lines {
                    let cmd = parse_zsh_history_line(&line);
                    if !cmd.is_empty() {
                        let is_ai = is_ai_command(&cmd);
                        let redacted = redact_command(&cmd);
                        let data = serde_json::json!({
                            "command": redacted,
                            "is_ai_tool": is_ai,
                        });
                        db::save_event(session_id, "terminal_command", &data.to_string());
                    }
                }
                last_line_count = current;
            }
        }

        // check running AI processes
        check_ai_processes(session_id);

        std::thread::sleep(Duration::from_secs(3));
    }
}

fn find_history_file() -> Option<String> {
    let home = dirs::home_dir()?;
    let zsh = home.join(".zsh_history");
    if zsh.exists() {
        return Some(zsh.to_string_lossy().to_string());
    }
    let bash = home.join(".bash_history");
    if bash.exists() {
        return Some(bash.to_string_lossy().to_string());
    }
    None
}

fn count_lines(path: &str) -> usize {
    Command::new("wc")
        .args(["-l", path])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0)
}

fn get_new_lines(path: &str, count: usize) -> Vec<String> {
    Command::new("tail")
        .args(["-n", &count.to_string(), path])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_zsh_history_line(line: &str) -> String {
    let trimmed = line.trim();
    // zsh extended history format: `: timestamp:0;command`
    if trimmed.starts_with(':') {
        if let Some(pos) = trimmed.find(';') {
            return trimmed[pos + 1..].to_string();
        }
    }
    trimmed.to_string()
}

/// Redact potentially sensitive values from commands.
/// Keeps the command name and structure, strips likely secrets.
fn redact_command(cmd: &str) -> String {
    let truncated: String = cmd.chars().take(200).collect();
    let lower = truncated.to_lowercase();

    // Inline env var assignment: VAR=value command (no export)
    // Match pattern: starts with UPPER_CASE=something
    if let Some(eq) = truncated.find('=') {
        let before = &truncated[..eq];
        if !before.is_empty()
            && !before.contains(' ')
            && before.chars().all(|c| c.is_ascii_uppercase() || c == '_')
        {
            return format!("{}=[REDACTED]", before);
        }
    }

    // export VAR=value
    if lower.starts_with("export ") && lower.contains('=') {
        let after_export = &truncated[7..]; // skip "export "
        if let Some(eq) = after_export.find('=') {
            return format!("export {}=[REDACTED]", &after_export[..eq]);
        }
    }

    // Redact values after sensitive flags (keep command name only)
    let sensitive_flags = &[
        "--password", "--token", "--secret", "--key", "--header",
        "-H", "-p", "-u",
    ];
    for flag in sensitive_flags {
        if lower.contains(flag) {
            let cmd_name = truncated.split_whitespace().next().unwrap_or(&truncated);
            return format!("{} [ARGS REDACTED]", cmd_name);
        }
    }

    // Connection strings with embedded credentials
    if lower.contains("://") && lower.contains('@') {
        let cmd_name = truncated.split_whitespace().next().unwrap_or(&truncated);
        return format!("{} [ARGS REDACTED]", cmd_name);
    }

    // Inline credential keywords
    if lower.contains("bearer ")
        || lower.contains("authorization:")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("password")
        || lower.contains("passwd")
    {
        let cmd_name = truncated.split_whitespace().next().unwrap_or(&truncated);
        return format!("{} [ARGS REDACTED]", cmd_name);
    }

    truncated
}

fn is_ai_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    AI_COMMANDS.iter().any(|ai| lower.contains(ai))
}

fn check_ai_processes(session_id: i64) {
    let output = match Command::new("ps").args(["aux"]).output() {
        Ok(o) => o,
        Err(_) => return,
    };

    let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
    let found: Vec<&str> = AI_PROCESSES
        .iter()
        .filter(|name| text.contains(**name))
        .copied()
        .collect();

    if !found.is_empty() {
        let data = serde_json::json!({
            "processes": found,
        });
        db::save_event(session_id, "ai_processes", &data.to_string());
    }
}
