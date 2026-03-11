use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::db;

pub fn run(session_id: i64, running: Arc<AtomicBool>) {
    let mut last_hash: u64 = get_clipboard_hash();

    while running.load(Ordering::SeqCst) {
        if let Some(content) = get_clipboard() {
            let h = hash_string(&content);
            if h != last_hash {
                last_hash = h;
                let is_code = looks_like_code(&content);
                let data = serde_json::json!({
                    "length": content.len(),
                    "is_code": is_code,
                });
                db::save_event(session_id, "clipboard", &data.to_string());
            }
        }

        std::thread::sleep(Duration::from_secs(1));
    }
}

fn get_clipboard() -> Option<String> {
    Command::new("pbpaste")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).to_string())
            } else {
                None
            }
        })
}

fn get_clipboard_hash() -> u64 {
    get_clipboard().map(|c| hash_string(&c)).unwrap_or(0)
}

fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn looks_like_code(text: &str) -> bool {
    const INDICATORS: &[&str] = &[
        "def ", "class ", "import ", "function ", "const ", "let ", "var ",
        "return ", "if (", "for (", "while (", "=>", "->", "(){", "};",
        "print(", "console.log", "<div", "</", "SELECT ", "FROM ",
    ];
    INDICATORS.iter().any(|ind| text.contains(ind))
}
