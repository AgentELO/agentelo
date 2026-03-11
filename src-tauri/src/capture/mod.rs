use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub mod clipboard;
pub mod filesystem;
pub mod keystrokes;
pub mod screenshot;
pub mod terminal;
pub mod window;

pub struct CaptureEngine {
    session_id: i64,
    running: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl CaptureEngine {
    pub fn new(session_id: i64) -> Self {
        Self {
            session_id,
            running: Arc::new(AtomicBool::new(false)),
            threads: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        self.running.store(true, Ordering::SeqCst);

        let sid = self.session_id;
        let r = self.running.clone();
        self.threads
            .push(std::thread::spawn(move || screenshot::run(sid, r)));

        let sid = self.session_id;
        let r = self.running.clone();
        self.threads
            .push(std::thread::spawn(move || window::run(sid, r)));

        let sid = self.session_id;
        let r = self.running.clone();
        self.threads
            .push(std::thread::spawn(move || clipboard::run(sid, r)));

        let sid = self.session_id;
        let r = self.running.clone();
        self.threads
            .push(std::thread::spawn(move || filesystem::run(sid, r)));

        let sid = self.session_id;
        let r = self.running.clone();
        self.threads
            .push(std::thread::spawn(move || terminal::run(sid, r)));

        let sid = self.session_id;
        let r = self.running.clone();
        self.threads
            .push(std::thread::spawn(move || keystrokes::run(sid, r)));
    }

    pub fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for CaptureEngine {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}
