use rusqlite::{Connection, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Once;

static SCHEMA_INIT: Once = Once::new();

fn db_path() -> PathBuf {
    let dir = dirs::home_dir().unwrap().join(".agentelo");
    std::fs::create_dir_all(&dir).ok();
    dir.join("agentelo.db")
}

pub fn get_conn() -> Result<Connection> {
    let conn = Connection::open(db_path())?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    SCHEMA_INIT.call_once(|| {
        init_schema(&conn).expect("Failed to initialize database schema");
    });
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            stopped_at TEXT,
            elo_before REAL NOT NULL DEFAULT 1200.0,
            elo_after REAL,
            percentile REAL,
            score_json TEXT,
            insights_json TEXT,
            gemini_score_json TEXT
        );
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            data TEXT,
            FOREIGN KEY (session_id) REFERENCES sessions(id)
        );
        CREATE TABLE IF NOT EXISTS badges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER,
            badge_name TEXT NOT NULL,
            earned_at TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id)
        );"
    )?;
    // Migration: add gemini_score_json if upgrading from older schema
    conn.execute_batch("ALTER TABLE sessions ADD COLUMN gemini_score_json TEXT;").ok();
    Ok(())
}

pub fn create_session(conn: &Connection) -> Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let (elo, _) = get_current_elo(conn);
    conn.execute(
        "INSERT INTO sessions (started_at, elo_before) VALUES (?1, ?2)",
        rusqlite::params![now, elo],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish_session(
    conn: &Connection,
    session_id: i64,
    elo_after: f64,
    percentile: f64,
    score_json: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE sessions SET stopped_at = ?1, elo_after = ?2, percentile = ?3, score_json = ?4 WHERE id = ?5",
        rusqlite::params![now, elo_after, percentile, score_json, session_id],
    )?;
    Ok(())
}

/// Update scores without touching stopped_at (used after Gemini scoring)
pub fn update_session_scores(
    conn: &Connection,
    session_id: i64,
    elo_after: f64,
    percentile: f64,
    score_json: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET elo_after = ?1, percentile = ?2, score_json = ?3 WHERE id = ?4",
        rusqlite::params![elo_after, percentile, score_json, session_id],
    )?;
    Ok(())
}

pub fn save_event(session_id: i64, event_type: &str, data: &str) {
    let conn = match get_conn() {
        Ok(c) => c,
        Err(_) => return,
    };
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "INSERT INTO events (session_id, event_type, timestamp, data) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![session_id, event_type, now, data],
    );
}

pub fn get_session_events(conn: &Connection, session_id: i64) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT event_type, timestamp, data FROM events WHERE session_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    rows.collect()
}

pub fn get_current_elo(conn: &Connection) -> (f64, Option<f64>) {
    let elo: f64 = conn
        .query_row(
            "SELECT elo_after FROM sessions WHERE elo_after IS NOT NULL ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(1200.0);

    let percentile: Option<f64> = conn
        .query_row(
            "SELECT percentile FROM sessions WHERE percentile IS NOT NULL ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    (elo, percentile)
}

pub fn count_sessions(conn: &Connection) -> usize {
    conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE stopped_at IS NOT NULL",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0) as usize
}

pub fn count_badges(conn: &Connection) -> usize {
    conn.query_row("SELECT COUNT(*) FROM badges", [], |row| {
        row.get::<_, i64>(0)
    })
    .unwrap_or(0) as usize
}

pub fn get_all_sessions(conn: &Connection) -> Result<Vec<super::SessionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, stopped_at, elo_before, elo_after, percentile, score_json
         FROM sessions ORDER BY id DESC LIMIT 50",
    )?;

    let rows = stmt.query_map([], |row| {
        let score_json: Option<String> = row.get(6)?;
        let overall_score = score_json.as_ref().and_then(|s| {
            serde_json::from_str::<serde_json::Value>(s)
                .ok()
                .and_then(|v| v.get("overall").and_then(|o| o.as_f64()))
        });

        Ok(super::SessionSummary {
            id: row.get(0)?,
            started_at: row.get(1)?,
            stopped_at: row.get(2)?,
            elo_before: row.get::<_, f64>(3).unwrap_or(1200.0),
            elo_after: row.get(4)?,
            percentile: row.get(5)?,
            overall_score,
        })
    })?;

    rows.collect()
}

pub fn get_session_detail(conn: &Connection, session_id: i64) -> Result<super::SessionDetail> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, stopped_at, elo_before, elo_after, percentile, score_json, insights_json, gemini_score_json
         FROM sessions WHERE id = ?",
    )?;

    let (session, score_json, insights_json, gemini_score_json) = stmt.query_row([session_id], |row| {
        let score_json: Option<String> = row.get(6)?;
        let insights_json: Option<String> = row.get(7)?;
        let gemini_score_json: Option<String> = row.get(8)?;

        // Prefer gemini overall score if available, fallback to local
        let overall_score = gemini_score_json
            .as_ref()
            .and_then(|s| {
                serde_json::from_str::<serde_json::Value>(s)
                    .ok()
                    .and_then(|v| v.get("overall").and_then(|o| o.as_f64()))
            })
            .or_else(|| {
                score_json.as_ref().and_then(|s| {
                    serde_json::from_str::<serde_json::Value>(s)
                        .ok()
                        .and_then(|v| v.get("overall").and_then(|o| o.as_f64()))
                })
            });

        Ok((
            super::SessionSummary {
                id: row.get(0)?,
                started_at: row.get(1)?,
                stopped_at: row.get(2)?,
                elo_before: row.get::<_, f64>(3).unwrap_or(1200.0),
                elo_after: row.get(4)?,
                percentile: row.get(5)?,
                overall_score,
            },
            score_json,
            insights_json,
            gemini_score_json,
        ))
    })?;

    let event_count: usize = conn.query_row(
        "SELECT COUNT(*) FROM events WHERE session_id = ?",
        [session_id],
        |row| row.get::<_, i64>(0),
    )? as usize;

    Ok(super::SessionDetail {
        session,
        score_json,
        insights_json,
        gemini_score_json,
        event_count,
    })
}

#[derive(Serialize)]
pub struct Badge {
    pub id: i64,
    pub session_id: Option<i64>,
    pub badge_name: String,
    pub earned_at: String,
}

pub fn get_all_badges(conn: &Connection) -> Result<Vec<Badge>> {
    let mut stmt =
        conn.prepare("SELECT id, session_id, badge_name, earned_at FROM badges ORDER BY earned_at DESC")?;

    let rows = stmt.query_map([], |row| {
        Ok(Badge {
            id: row.get(0)?,
            session_id: row.get(1)?,
            badge_name: row.get(2)?,
            earned_at: row.get(3)?,
        })
    })?;

    rows.collect()
}

pub fn cleanup_orphaned_sessions(conn: &Connection) -> usize {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE sessions SET stopped_at = ?1, score_json = '{\"overall\":0,\"error\":\"orphaned\"}' \
         WHERE stopped_at IS NULL",
        rusqlite::params![now],
    )
    .unwrap_or(0)
}

pub fn get_screenshot_paths(conn: &Connection, session_id: i64) -> Vec<String> {
    let events = get_session_events(conn, session_id).unwrap_or_default();
    events
        .iter()
        .filter(|(etype, _, _)| etype == "screenshot")
        .filter_map(|(_, _, data)| {
            serde_json::from_str::<serde_json::Value>(data)
                .ok()
                .and_then(|v| v.get("path").and_then(|p| p.as_str()).map(|s| s.to_string()))
        })
        .collect()
}

pub fn save_insights(conn: &Connection, session_id: i64, insights_json: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET insights_json = ?1 WHERE id = ?2",
        rusqlite::params![insights_json, session_id],
    )?;
    Ok(())
}

pub fn save_gemini_scores(conn: &Connection, session_id: i64, gemini_scores: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET gemini_score_json = ?1 WHERE id = ?2",
        rusqlite::params![gemini_scores, session_id],
    )?;
    Ok(())
}

pub fn build_events_summary(conn: &Connection, session_id: i64) -> Option<String> {
    let events = get_session_events(conn, session_id).ok()?;
    if events.is_empty() {
        return None;
    }

    let mut window_switches = 0u32;
    let mut file_changes = 0u32;
    let mut terminal_cmds = 0u32;
    let mut clipboard_events = 0u32;
    let mut ai_processes = 0u32;
    let mut keystroke_samples = 0u32;
    let mut apps_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut ai_tools: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut sample_cmds: Vec<String> = Vec::new();

    for (event_type, _ts, data) in &events {
        let json: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
        match event_type.as_str() {
            "window_change" | "window_active" => {
                window_switches += 1;
                if let Some(app) = json.get("app").and_then(|v| v.as_str()) {
                    apps_seen.insert(app.to_string());
                }
            }
            "file_change" => file_changes += 1,
            "terminal_command" => {
                terminal_cmds += 1;
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    if sample_cmds.len() < 10 {
                        sample_cmds.push(cmd.chars().take(80).collect());
                    }
                }
            }
            "clipboard" => clipboard_events += 1,
            "ai_processes" => {
                ai_processes += 1;
                if let Some(procs) = json.get("processes").and_then(|v| v.as_array()) {
                    for p in procs {
                        if let Some(name) = p.as_str() {
                            ai_tools.insert(name.to_string());
                        }
                    }
                }
            }
            "keystroke_velocity" => keystroke_samples += 1,
            _ => {}
        }
    }

    let mut summary = format!(
        "Total events: {}\n\
         Window switches: {}\n\
         File changes: {}\n\
         Terminal commands: {}\n\
         Clipboard events: {}\n\
         AI process snapshots: {}\n\
         Keystroke samples: {}\n\
         Apps used: {}\n\
         AI tools detected: {}",
        events.len(),
        window_switches,
        file_changes,
        terminal_cmds,
        clipboard_events,
        ai_processes,
        keystroke_samples,
        apps_seen.into_iter().collect::<Vec<_>>().join(", "),
        if ai_tools.is_empty() {
            "none".to_string()
        } else {
            ai_tools.into_iter().collect::<Vec<_>>().join(", ")
        },
    );

    if !sample_cmds.is_empty() {
        summary.push_str("\nSample commands:\n");
        for cmd in &sample_cmds {
            summary.push_str(&format!("  - {}\n", cmd));
        }
    }

    Some(summary)
}

