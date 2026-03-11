use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::db;

pub fn run(session_id: i64, running: Arc<AtomicBool>) {
    let frames_dir = dirs::home_dir()
        .unwrap()
        .join(".agentelo/frames")
        .join(session_id.to_string());
    std::fs::create_dir_all(&frames_dir).ok();

    let mut frame_count: u32 = 0;

    while running.load(Ordering::SeqCst) {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("frame_{:05}_{}.jpg", frame_count, ts);
        let filepath = frames_dir.join(&filename);

        let result = Command::new("screencapture")
            .args(["-x", "-t", "jpg", "-C", filepath.to_str().unwrap()])
            .output();

        if result.is_ok() && filepath.exists() {
            let data = serde_json::json!({
                "frame": frame_count,
                "path": filepath.to_str().unwrap(),
            });
            db::save_event(session_id, "screenshot", &data.to_string());
            frame_count += 1;
        }

        std::thread::sleep(Duration::from_secs(5));
    }
}
