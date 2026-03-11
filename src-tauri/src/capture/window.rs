use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::db;

pub fn run(session_id: i64, running: Arc<AtomicBool>) {
    let mut last_window: Option<String> = None;

    while running.load(Ordering::SeqCst) {
        if let Some((app, title)) = get_active_window() {
            let key = format!("{}|||{}", app, title);
            let event_type = if last_window.as_deref() != Some(&key) {
                last_window = Some(key);
                "window_change"
            } else {
                "window_active"
            };

            let data = serde_json::json!({
                "app": app,
                "title": title,
            });
            db::save_event(session_id, event_type, &data.to_string());
        }

        std::thread::sleep(Duration::from_secs(2));
    }
}

fn get_active_window() -> Option<(String, String)> {
    let script = r#"tell application "System Events"
        set frontApp to name of first application process whose frontmost is true
        return frontApp
    end tell"#;

    let result = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if !result.status.success() {
        return None;
    }

    let app_name = String::from_utf8_lossy(&result.stdout).trim().to_string();
    let title = get_title_for_app(&app_name);
    Some((app_name, title))
}

fn get_title_for_app(app_name: &str) -> String {
    let app_lower = app_name.to_lowercase();

    let script = if app_lower == "terminal" {
        r#"tell application "Terminal"
            try
                return name of front window
            end try
            return ""
        end tell"#
            .to_string()
    } else if app_lower.contains("iterm") {
        r#"tell application "iTerm2"
            try
                return name of current session of current tab of current window
            end try
            return ""
        end tell"#
            .to_string()
    } else if app_lower == "google chrome" {
        r#"tell application "Google Chrome"
            try
                return title of active tab of front window
            end try
            return ""
        end tell"#
            .to_string()
    } else if app_lower == "safari" {
        r#"tell application "Safari"
            try
                return name of current tab of front window
            end try
            return ""
        end tell"#
            .to_string()
    } else if app_lower == "arc" {
        r#"tell application "Arc"
            try
                return title of active tab of front window
            end try
            return ""
        end tell"#
            .to_string()
    } else {
        let safe_name = app_name.replace('"', "").replace('\\', "");
        format!(
            r#"tell application "System Events"
            try
                return name of front window of (first application process whose name is "{}")
            end try
            return ""
        end tell"#,
            safe_name
        )
    };

    run_applescript(&script)
}

fn run_applescript(script: &str) -> String {
    Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}
