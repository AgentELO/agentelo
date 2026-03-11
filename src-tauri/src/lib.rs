use serde::Serialize;
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    Manager, State,
};

use base64::Engine;

mod analysis;
mod auth;
mod capture;
mod db;
mod sync;

/// Sample every 6th screenshot (30s intervals from 5s captures),
/// resize to 512px wide, return as base64 JPEG strings. Max 20 frames.
fn sample_screenshots(paths: &[String]) -> Vec<String> {
    let step = 6; // every 6th = 30s intervals
    let max_frames = 20;

    paths
        .iter()
        .step_by(step)
        .take(max_frames)
        .filter_map(|path| {
            let img = image::open(path).ok()?;
            let resized = img.resize(512, 512, image::imageops::FilterType::Triangle);
            let mut buf = std::io::Cursor::new(Vec::new());
            resized
                .write_to(&mut buf, image::ImageFormat::Jpeg)
                .ok()?;
            Some(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// State — single Mutex for recording fields to prevent deadlock
// ---------------------------------------------------------------------------

struct RecordingState {
    active: bool,
    session_id: Option<i64>,
    engine: Option<capture::CaptureEngine>,
}

struct AppState {
    recording: Mutex<RecordingState>,
    sync_client: Mutex<sync::SyncClient>,
    user: Mutex<Option<sync::UserInfo>>,
    tray: Mutex<Option<TrayIcon>>,
    tray_record_item: Mutex<Option<MenuItem<tauri::Wry>>>,
    tray_elo_item: Mutex<Option<MenuItem<tauri::Wry>>>,
    tray_icon_default: Image<'static>,
    tray_icon_recording: Image<'static>,
}

// ---------------------------------------------------------------------------
// IPC types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SessionSummary {
    id: i64,
    started_at: String,
    stopped_at: Option<String>,
    elo_before: f64,
    elo_after: Option<f64>,
    percentile: Option<f64>,
    overall_score: Option<f64>,
}

#[derive(Serialize)]
struct CurrentStatus {
    recording: bool,
    session_id: Option<i64>,
    current_elo: f64,
    percentile: Option<f64>,
    total_sessions: usize,
    total_badges: usize,
}

#[derive(Serialize)]
struct SessionDetail {
    session: SessionSummary,
    score_json: Option<String>,
    insights_json: Option<String>,
    gemini_score_json: Option<String>,
    event_count: usize,
}

// ---------------------------------------------------------------------------
// Local data commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_status(state: State<AppState>) -> Result<CurrentStatus, String> {
    let conn = db::get_conn().map_err(|e| e.to_string())?;
    let rec = state.recording.lock().unwrap();

    let (current_elo, percentile) = db::get_current_elo(&conn);
    let total_sessions = db::count_sessions(&conn);
    let total_badges = db::count_badges(&conn);

    Ok(CurrentStatus {
        recording: rec.active,
        session_id: rec.session_id,
        current_elo,
        percentile,
        total_sessions,
        total_badges,
    })
}

#[tauri::command]
fn get_sessions() -> Result<Vec<SessionSummary>, String> {
    let conn = db::get_conn().map_err(|e| e.to_string())?;
    db::get_all_sessions(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_session_detail(session_id: i64) -> Result<SessionDetail, String> {
    let conn = db::get_conn().map_err(|e| e.to_string())?;
    db::get_session_detail(&conn, session_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_badges() -> Result<Vec<db::Badge>, String> {
    let conn = db::get_conn().map_err(|e| e.to_string())?;
    db::get_all_badges(&conn).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Recording commands
// ---------------------------------------------------------------------------

/// Shared start-recording logic used by both the Tauri command and tray handler.
fn do_start_recording(app: &tauri::AppHandle) -> Result<i64, String> {
    let state = app.state::<AppState>();
    let mut rec = state.recording.lock().unwrap();
    if rec.active {
        return Err("Already recording".to_string());
    }

    let conn = db::get_conn().map_err(|e| e.to_string())?;
    let session_id = db::create_session(&conn).map_err(|e| e.to_string())?;

    let mut engine = capture::CaptureEngine::new(session_id);
    engine.start();

    rec.engine = Some(engine);
    rec.session_id = Some(session_id);
    rec.active = true;

    update_tray_for_recording(app, true);

    Ok(session_id)
}

#[tauri::command]
fn start_recording(app: tauri::AppHandle, _state: State<AppState>) -> Result<String, String> {
    let session_id = do_start_recording(&app)?;
    Ok(format!("Recording started (session {})", session_id))
}

/// Shared stop-recording logic used by both the Tauri command and tray handler.
async fn do_stop_recording(app: &tauri::AppHandle) -> Result<Option<i64>, String> {
    let state = app.state::<AppState>();
    let (engine, session_id) = {
        let mut rec = state.recording.lock().unwrap();
        if !rec.active {
            return Err("Not recording".to_string());
        }
        rec.active = false;
        (rec.engine.take(), rec.session_id.take())
    };

    update_tray_for_recording(app, false);

    if let Some(engine) = engine {
        tokio::task::spawn_blocking(move || engine.stop())
            .await
            .map_err(|e| format!("Failed to stop capture: {e}"))?;
    }

    let Some(sid) = session_id else {
        return Ok(None);
    };

    // Finish session in DB with no score yet (Gemini will provide it)
    let elo_before = tokio::task::spawn_blocking(move || {
        let conn = db::get_conn().map_err(|e| e.to_string())?;
        let (current_elo, _) = db::get_current_elo(&conn);
        db::finish_session(&conn, sid, current_elo, 50.0, "")
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(current_elo)
    })
    .await
    .map_err(|e| format!("DB error: {e}"))??;

    // Score the session: BYOK (local Gemini) or cloud
    let client = state.sync_client.lock().unwrap().clone();
    let byok_key = auth::load_api_key();

    if let Some(key_config) = byok_key {
        // BYOK: score locally, then sync scores-only to leaderboard
        let sync_sid = sid;
        let app_for_sync = app.clone();
        tokio::spawn(async move {
            score_locally_and_sync(app_for_sync, client, sync_sid, elo_before, key_config).await;
        });
    } else if client.has_token() {
        // Cloud: upload screenshots + events to backend for Gemini scoring
        let sync_sid = sid;
        let app_for_sync = app.clone();
        tokio::spawn(async move {
            upload_and_score(app_for_sync, client, sync_sid, elo_before).await;
        });
    }

    Ok(Some(sid))
}

/// Upload session data to backend and apply Gemini scores.
async fn upload_and_score(
    app: tauri::AppHandle,
    client: sync::SyncClient,
    sid: i64,
    elo_before: f64,
) {
    let upload_data = tokio::task::spawn_blocking(move || {
        let conn = db::get_conn().ok()?;
        let summary = db::build_events_summary(&conn, sid);
        let screenshot_paths = db::get_screenshot_paths(&conn, sid);
        let frames = sample_screenshots(&screenshot_paths);
        let started_at = conn
            .query_row(
                "SELECT started_at FROM sessions WHERE id = ?",
                [sid],
                |row| row.get::<_, String>(0),
            )
            .ok()?;
        let stopped_at = conn
            .query_row(
                "SELECT stopped_at FROM sessions WHERE id = ?",
                [sid],
                |row| row.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()?;
        Some((started_at, stopped_at, summary, frames))
    })
    .await
    .ok()
    .flatten();

    let Some((started_at, stopped_at, events_summary, frames)) = upload_data else {
        return;
    };

    let duration = chrono::DateTime::parse_from_rfc3339(&stopped_at)
        .ok()
        .and_then(|stop| {
            chrono::DateTime::parse_from_rfc3339(&started_at)
                .ok()
                .map(|start| (stop - start).num_seconds())
        })
        .unwrap_or(0);

    let upload = sync::SessionUpload {
        started_at,
        stopped_at,
        duration_sec: duration,
        elo_before,
        elo_after: elo_before,
        score_json: None,
        events_summary_json: events_summary,
        screenshot_frames: frames,
    };

    if let Ok(resp) = client.upload_session(&upload).await {
        let insights = resp.insights_json.clone();
        let gemini_scores = resp.gemini_scores.clone();
        let percentile = resp.percentile;
        let sync_sid = sid;
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(conn) = db::get_conn() {
                if let Some(insights) = &insights {
                    let _ = db::save_insights(&conn, sync_sid, insights);
                }
                if let Some(scores) = &gemini_scores {
                    let scores_str = serde_json::to_string(scores).unwrap_or_default();
                    let _ = db::save_gemini_scores(&conn, sync_sid, &scores_str);
                    if let Some(overall) = scores.get("overall").and_then(|v| v.as_f64()) {
                        let new_elo =
                            analysis::scorer::calculate_elo(elo_before, overall, 32.0);
                        let _ = db::update_session_scores(
                            &conn, sync_sid, new_elo, percentile, &scores_str,
                        );
                    }
                }
            }
        })
        .await;
        update_tray_elo(&app);
    }
}

/// BYOK: score session locally using user's own Gemini key, then sync scores-only.
async fn score_locally_and_sync(
    app: tauri::AppHandle,
    client: sync::SyncClient,
    sid: i64,
    elo_before: f64,
    key_config: auth::ApiKeyConfig,
) {
    // 1. Build events summary + sample screenshots (same prep as upload_and_score)
    let upload_data = tokio::task::spawn_blocking(move || {
        let conn = db::get_conn().ok()?;
        let summary = db::build_events_summary(&conn, sid);
        let screenshot_paths = db::get_screenshot_paths(&conn, sid);
        let frames = sample_screenshots(&screenshot_paths);
        let started_at = conn
            .query_row(
                "SELECT started_at FROM sessions WHERE id = ?",
                [sid],
                |row| row.get::<_, String>(0),
            )
            .ok()?;
        let stopped_at = conn
            .query_row(
                "SELECT stopped_at FROM sessions WHERE id = ?",
                [sid],
                |row| row.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()?;
        Some((started_at, stopped_at, summary, frames))
    })
    .await
    .ok()
    .flatten();

    let Some((started_at, stopped_at, events_summary, frames)) = upload_data else {
        return;
    };

    let summary_text = events_summary.as_deref().unwrap_or("No events captured");

    // 2. Call Gemini directly with user's own key
    let gemini_result =
        analysis::gemini::score_session_locally(&key_config.api_key, summary_text, &frames).await;

    let sync_sid = sid;
    match gemini_result {
        Ok(result) => {
            let scores = result.scores.clone();
            let insights = result.insights_markdown.clone();
            let scores_for_sync = result.scores.clone();

            // 3. Save scores + insights to local SQLite
            let new_elo = tokio::task::spawn_blocking(move || {
                let conn = match db::get_conn() {
                    Ok(c) => c,
                    Err(_) => return elo_before,
                };
                let _ = db::save_insights(&conn, sync_sid, &insights);
                let scores_str = serde_json::to_string(&scores).unwrap_or_default();
                let _ = db::save_gemini_scores(&conn, sync_sid, &scores_str);

                // 4. Calculate ELO locally
                let overall = scores.get("overall").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let new_elo = analysis::scorer::calculate_elo(elo_before, overall, 32.0);
                let _ = db::update_session_scores(&conn, sync_sid, new_elo, 50.0, &scores_str);
                new_elo
            })
            .await
            .unwrap_or(elo_before);

            update_tray_elo(&app);

            // 5. Sync scores-only to leaderboard (no screenshots, no events)
            if client.has_token() {
                let duration = chrono::DateTime::parse_from_rfc3339(&stopped_at)
                    .ok()
                    .and_then(|stop| {
                        chrono::DateTime::parse_from_rfc3339(&started_at)
                            .ok()
                            .map(|start| (stop - start).num_seconds())
                    })
                    .unwrap_or(0);

                let scores_str =
                    serde_json::to_string(&scores_for_sync).unwrap_or_default();
                let upload = sync::SessionUpload {
                    started_at,
                    stopped_at,
                    duration_sec: duration,
                    elo_before,
                    elo_after: new_elo,
                    score_json: Some(scores_str),
                    events_summary_json: None, // intentionally omit — prevents backend re-scoring
                    screenshot_frames: vec![], // never send screenshots in BYOK mode
                };
                // Fire and forget — don't overwrite local data from response
                let _ = client.upload_session(&upload).await;
            }
        }
        Err(e) => {
            // Save error so UI can display it
            eprintln!("BYOK Gemini scoring failed: {e}");
            let error_json = serde_json::json!({"error": e}).to_string();
            let _ = tokio::task::spawn_blocking(move || {
                if let Ok(conn) = db::get_conn() {
                    let _ = db::update_session_scores(&conn, sync_sid, elo_before, 50.0, &error_json);
                }
            })
            .await;
        }
    }
}

#[tauri::command]
async fn stop_recording(app: tauri::AppHandle, _state: State<'_, AppState>) -> Result<String, String> {
    match do_stop_recording(&app).await {
        Ok(Some(sid)) => Ok(format!("Recording stopped (session {sid})")),
        Ok(None) => Ok("Recording stopped".to_string()),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Auth commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn login(state: State<'_, AppState>) -> Result<sync::UserInfo, String> {
    let api_base = state.sync_client.lock().unwrap().api_base().to_string();
    let stored = auth::run_oauth_flow(&api_base).await?;

    state
        .sync_client
        .lock()
        .unwrap()
        .set_token(stored.token.clone());
    *state.user.lock().unwrap() = Some(stored.user.clone());

    Ok(stored.user)
}

#[tauri::command]
fn logout(state: State<AppState>) -> Result<(), String> {
    // Clear Google-linked auth, but keep device auth working
    state.sync_client.lock().unwrap().clear_token();
    *state.user.lock().unwrap() = None;
    auth::clear_auth();
    Ok(())
}

#[tauri::command]
fn get_auth_state(state: State<AppState>) -> Result<Option<sync::UserInfo>, String> {
    Ok(state.user.lock().unwrap().clone())
}

// ---------------------------------------------------------------------------
// BYOK — Gemini API key management
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ApiKeyConfigResponse {
    masked_key: String,
}

#[tauri::command]
fn get_api_key_config() -> Option<ApiKeyConfigResponse> {
    let config = auth::load_api_key()?;
    let key = &config.api_key;
    let masked = if key.len() > 4 {
        format!("...{}", &key[key.len() - 4..])
    } else {
        "****".to_string()
    };
    Some(ApiKeyConfigResponse { masked_key: masked })
}

#[tauri::command]
fn save_api_key_config(api_key: String) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    auth::save_api_key(&auth::ApiKeyConfig {
        api_key: api_key.trim().to_string(),
    });
    Ok(())
}

#[tauri::command]
fn clear_api_key_config() -> Result<(), String> {
    auth::clear_api_key();
    Ok(())
}

// ---------------------------------------------------------------------------
// Cloud sync commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_leaderboard_data(
    state: State<'_, AppState>,
) -> Result<Vec<sync::LeaderboardEntry>, String> {
    let client = state.sync_client.lock().unwrap().clone();
    client.get_leaderboard(50).await
}

#[tauri::command]
async fn sync_session(
    session_id: i64,
    state: State<'_, AppState>,
) -> Result<sync::SessionUploadResponse, String> {
    let client = state.sync_client.lock().unwrap().clone();
    if !client.has_token() {
        return Err("Not logged in".to_string());
    }

    // BYOK guard: never send screenshots or events if user has their own Gemini key
    let is_byok = auth::load_api_key().is_some();

    let (detail, events_summary, frames) = tokio::task::spawn_blocking(move || {
        let conn = db::get_conn().map_err(|e| e.to_string())?;
        let d = db::get_session_detail(&conn, session_id).map_err(|e| e.to_string())?;
        if is_byok {
            // BYOK: scores only, no screenshots or events
            Ok::<_, String>((d, None, vec![]))
        } else {
            let summary = db::build_events_summary(&conn, session_id);
            let paths = db::get_screenshot_paths(&conn, session_id);
            let frames = sample_screenshots(&paths);
            Ok::<_, String>((d, summary, frames))
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    let session = &detail.session;

    let stopped = session
        .stopped_at
        .as_ref()
        .ok_or("Cannot sync an incomplete session")?;
    let started = &session.started_at;
    let elo_before = session.elo_before;
    let elo_after = session.elo_after.unwrap_or(elo_before);

    let duration = chrono::DateTime::parse_from_rfc3339(stopped)
        .ok()
        .and_then(|stop| {
            chrono::DateTime::parse_from_rfc3339(started)
                .ok()
                .map(|start| (stop - start).num_seconds())
        })
        .unwrap_or(0);

    let upload = sync::SessionUpload {
        started_at: started.clone(),
        stopped_at: stopped.clone(),
        duration_sec: duration,
        elo_before,
        elo_after,
        score_json: detail.score_json.clone(),
        events_summary_json: events_summary,
        screenshot_frames: frames,
    };

    client.upload_session(&upload).await
}

// ---------------------------------------------------------------------------
// Update check
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct UpdateInfo {
    available: bool,
    version: Option<String>,
    download_url: Option<String>,
}

#[tauri::command]
async fn check_for_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION");
    let client = reqwest::Client::new();
    let resp = client
        .get("https://agentelo.ai/api/update/latest")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Ok(UpdateInfo {
            available: false,
            version: None,
            download_url: None,
        });
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let latest = data
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or(current);
    let download_url = data
        .get("download_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let available = latest != current;
    Ok(UpdateInfo {
        available,
        version: if available {
            Some(latest.to_string())
        } else {
            None
        },
        download_url,
    })
}

// ---------------------------------------------------------------------------
// Window management
// ---------------------------------------------------------------------------

fn update_tray_for_recording(app: &tauri::AppHandle, recording: bool) {
    let state = app.state::<AppState>();
    {
        let guard = state.tray_record_item.lock().unwrap();
        if let Some(item) = guard.as_ref() {
            let _ = item.set_text(if recording { "Stop Recording" } else { "Start Recording" });
        }
    }
    let icon = if recording {
        state.tray_icon_recording.clone()
    } else {
        state.tray_icon_default.clone()
    };
    let tooltip = if recording { "AgentELO — Recording" } else { "AgentELO" };
    let guard = state.tray.lock().unwrap();
    if let Some(tray) = guard.as_ref() {
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn update_tray_elo(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(conn) = db::get_conn() {
        let (elo, _) = db::get_current_elo(&conn);
        if let Some(item) = state.tray_elo_item.lock().unwrap().as_ref() {
            let _ = item.set_text(format!("ELO: {}", elo.round() as i64));
        }
    }
}

fn handle_tray_record(app: &tauri::AppHandle) {
    let is_recording = app.state::<AppState>().recording.lock().unwrap().active;

    if is_recording {
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = do_stop_recording(&app_handle).await;
        });
    } else {
        let _ = do_start_recording(app);
    }
}

async fn do_device_auth(app: &tauri::AppHandle) {
    let device_id = auth::get_device_id();
    let client = app
        .state::<AppState>()
        .sync_client
        .lock()
        .unwrap()
        .clone();

    match client.device_auth(&device_id).await {
        Ok(resp) => {
            app.state::<AppState>()
                .sync_client
                .lock()
                .unwrap()
                .set_token(resp.token.clone());
            *app.state::<AppState>().user.lock().unwrap() = Some(resp.user.clone());
            auth::save_auth(&auth::StoredAuth {
                token: resp.token,
                user: resp.user,
            });
        }
        Err(e) => {
            eprintln!("Device auth failed (offline?): {e}");
            // App still works locally, just won't sync/score
        }
    }
}

fn open_dashboard(app: &tauri::AppHandle) {
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("dashboard") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        let _ = tauri::WebviewWindowBuilder::new(
            app,
            "dashboard",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("AgentELO")
        .inner_size(960.0, 680.0)
        .min_inner_size(800.0, 600.0)
        .decorations(true)
        .build();
    }
}

// ---------------------------------------------------------------------------
// App entry
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Restore auth state from disk
    let (sync_client, user) = match auth::load_stored_auth() {
        Some(stored) => {
            let mut client = sync::SyncClient::new();
            client.set_token(stored.token);
            (client, Some(stored.user))
        }
        None => (sync::SyncClient::new(), None),
    };

    // Clean up orphaned sessions (started but never stopped, e.g. after crash)
    if let Ok(conn) = db::get_conn() {
        let cleaned = db::cleanup_orphaned_sessions(&conn);
        if cleaned > 0 {
            eprintln!("Cleaned up {cleaned} orphaned session(s)");
        }
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState {
            recording: Mutex::new(RecordingState {
                active: false,
                session_id: None,
                engine: None,
            }),
            sync_client: Mutex::new(sync_client),
            user: Mutex::new(user),
            tray: Mutex::new(None),
            tray_record_item: Mutex::new(None),
            tray_elo_item: Mutex::new(None),
            tray_icon_default: Image::from_bytes(include_bytes!("../icons/icon.png"))
                .expect("failed to load default tray icon"),
            tray_icon_recording: Image::from_bytes(include_bytes!("../icons/recording.png"))
                .expect("failed to load recording tray icon"),
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_sessions,
            get_session_detail,
            get_badges,
            start_recording,
            stop_recording,
            login,
            logout,
            get_auth_state,
            get_api_key_config,
            save_api_key_config,
            clear_api_key_config,
            get_leaderboard_data,
            sync_session,
            check_for_update,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "dashboard" {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
        })
        .setup(|app| {
            // Tray-only: hide from dock on startup
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let record_item = MenuItem::with_id(app, "record", "Start Recording", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let elo_item = MenuItem::with_id(app, "elo", "ELO: 1,200", false, None::<&str>)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let dashboard = MenuItem::with_id(app, "dashboard", "Open Dashboard", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit AgentELO", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&record_item, &sep1, &elo_item, &sep2, &dashboard, &quit])?;

            // Set initial ELO from local DB
            if let Ok(conn) = db::get_conn() {
                let (elo, _) = db::get_current_elo(&conn);
                let _ = elo_item.set_text(format!("ELO: {}", elo.round() as i64));
            }

            let default_icon = app.state::<AppState>().tray_icon_default.clone();
            let tray = TrayIconBuilder::new()
                .icon(default_icon)
                .menu(&menu)
                .tooltip("AgentELO")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            // Stop recording gracefully before quitting
                            let _ = do_stop_recording(&app_handle).await;
                            app_handle.exit(0);
                        });
                    }
                    "dashboard" => {
                        open_dashboard(app);
                    }
                    "record" => {
                        handle_tray_record(app);
                    }
                    _ => {}
                })
                .show_menu_on_left_click(true)
                .build(app)?;

            *app.state::<AppState>().tray.lock().unwrap() = Some(tray);
            *app.state::<AppState>().tray_record_item.lock().unwrap() = Some(record_item);
            *app.state::<AppState>().tray_elo_item.lock().unwrap() = Some(elo_item);

            // Auto-auth: validate existing token or register with device ID
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let has_token = app_handle
                    .state::<AppState>()
                    .sync_client
                    .lock()
                    .unwrap()
                    .has_token();

                if has_token {
                    // Validate existing token
                    let client = app_handle
                        .state::<AppState>()
                        .sync_client
                        .lock()
                        .unwrap()
                        .clone();

                    if let Ok(me) = client.get_me().await {
                        *app_handle.state::<AppState>().user.lock().unwrap() =
                            Some(me.user);
                    } else {
                        // Token expired — re-auth with device ID
                        eprintln!("Stored token invalid, re-authenticating with device ID");
                        do_device_auth(&app_handle).await;
                    }
                } else {
                    // No token — first launch or cleared, auth with device ID
                    do_device_auth(&app_handle).await;
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = &event {
            // Only keep running if exit was triggered by last window closing (code None),
            // not by an explicit app.exit() call (code Some)
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}
