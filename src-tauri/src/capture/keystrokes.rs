use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::db;

static KEY_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn run(session_id: i64, running: Arc<AtomicBool>) {
    let has_tap = start_event_tap(running.clone());
    let mut samples: VecDeque<f64> = VecDeque::with_capacity(60);

    while running.load(Ordering::SeqCst) {
        let kpm = if has_tap {
            let keys = KEY_COUNT.swap(0, Ordering::SeqCst);
            (keys as f64 / 5.0) * 60.0
        } else {
            0.0
        };

        samples.push_back(kpm);
        if samples.len() > 60 {
            samples.pop_front();
        }

        let pattern = detect_pattern(&samples);

        let data = serde_json::json!({
            "keys_per_minute": (kpm * 10.0).round() / 10.0,
            "pattern": pattern,
            "has_cgevent": has_tap,
        });
        db::save_event(session_id, "keystroke_velocity", &data.to_string());

        std::thread::sleep(Duration::from_secs(5));
    }
}

fn detect_pattern(samples: &VecDeque<f64>) -> &'static str {
    if samples.len() < 3 {
        return "insufficient_data";
    }

    let recent: Vec<f64> = samples.iter().rev().take(6).copied().collect();
    let avg: f64 = recent.iter().sum::<f64>() / recent.len() as f64;

    if avg > 200.0 {
        "steady_typing"
    } else if avg < 10.0 {
        "idle_or_reading"
    } else if recent.len() >= 4 {
        let last = recent[0];
        let prev_avg: f64 = recent[1..].iter().sum::<f64>() / (recent.len() - 1) as f64;
        if last > prev_avg * 3.0 && prev_avg < 50.0 {
            "burst_after_pause"
        } else {
            "moderate_typing"
        }
    } else {
        "moderate_typing"
    }
}

#[cfg(target_os = "macos")]
mod cg_tap {
    use std::ffi::c_void;
    use std::sync::atomic::Ordering;

    // CGEventTap FFI types
    type CGEventRef = *mut c_void;
    type CGEventTapProxy = *mut c_void;
    type CGEventMask = u64;
    type CFMachPortRef = *mut c_void;
    type CFRunLoopSourceRef = *mut c_void;
    type CFRunLoopRef = *mut c_void;
    type CFStringRef = *const c_void;

    #[allow(non_upper_case_globals)]
    const kCGSessionEventTap: u32 = 1;
    #[allow(non_upper_case_globals)]
    const kCGHeadInsertEventTap: u32 = 0;
    #[allow(non_upper_case_globals)]
    const kCGEventTapOptionListenOnly: u32 = 1;
    #[allow(non_upper_case_globals)]
    const kCGEventKeyDown: u32 = 10;

    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: CGEventMask,
            callback: extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;
        fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
        fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source_handled: u8) -> i32;
        fn CGEventTapEnable(tap: CFMachPortRef, enable: u8);

        static kCFRunLoopCommonModes: CFStringRef;
        static kCFRunLoopDefaultMode: CFStringRef;
    }

    extern "C" fn tap_callback(
        _proxy: CGEventTapProxy,
        _event_type: u32,
        event: CGEventRef,
        _user_info: *mut c_void,
    ) -> CGEventRef {
        super::KEY_COUNT.fetch_add(1, Ordering::SeqCst);
        event
    }

    pub fn create_tap() -> bool {
        unsafe {
            let mask: CGEventMask = 1 << kCGEventKeyDown;
            let tap = CGEventTapCreate(
                kCGSessionEventTap,
                kCGHeadInsertEventTap,
                kCGEventTapOptionListenOnly,
                mask,
                tap_callback,
                std::ptr::null_mut(),
            );
            if tap.is_null() {
                return false;
            }
            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                return false;
            }
            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, 1);
            true
        }
    }

    pub fn run_loop_tick() {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0, 0);
        }
    }
}

fn start_event_tap(running: Arc<AtomicBool>) -> bool {
    #[cfg(target_os = "macos")]
    {
        // Spawn a thread that creates the tap and runs the CFRunLoop
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let ok = cg_tap::create_tap();
            let _ = tx.send(ok);
            if ok {
                while running.load(Ordering::SeqCst) {
                    cg_tap::run_loop_tick();
                }
            }
        });
        rx.recv().unwrap_or(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = running;
        false
    }
}
