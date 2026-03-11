use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone)]
pub struct SyncClient {
    http: Client,
    api_base: String,
    token: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct UserInfo {
    pub id: i64,
    pub google_id: Option<String>,
    pub email: Option<String>,
    pub name: String,
    pub avatar_url: Option<String>,
    pub current_elo: f64,
    pub total_sessions: i64,
}

#[derive(Serialize)]
struct DeviceAuthPayload {
    device_id: String,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Serialize)]
pub struct SessionUpload {
    pub started_at: String,
    pub stopped_at: String,
    pub duration_sec: i64,
    pub elo_before: f64,
    pub elo_after: f64,
    pub score_json: Option<String>,
    pub events_summary_json: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub screenshot_frames: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct SessionUploadResponse {
    pub session_id: i64,
    pub percentile: f64,
    pub insights_json: Option<String>,
    pub gemini_scores: Option<serde_json::Value>,
    pub badges: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct LeaderboardEntry {
    pub rank: i64,
    pub name: String,
    pub avatar_url: Option<String>,
    pub elo: f64,
    pub total_sessions: i64,
    pub trend: f64,
}

#[derive(Deserialize, Serialize)]
pub struct MeResponse {
    pub user: UserInfo,
    pub rank: i64,
    pub percentile: f64,
}

fn api_base() -> String {
    option_env!("AGENTELO_API_BASE")
        .unwrap_or("https://agentelo.ai/api/elo")
        .to_string()
}

fn build_http_client() -> Client {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .build()
        .expect("failed to build HTTP client")
}

impl SyncClient {
    pub fn new() -> Self {
        Self {
            http: build_http_client(),
            api_base: api_base(),
            token: None,
        }
    }

    pub fn with_token(api_base: &str, token: &str) -> Self {
        Self {
            http: build_http_client(),
            api_base: api_base.to_string(),
            token: Some(token.to_string()),
        }
    }

    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    pub fn clear_token(&mut self) {
        self.token = None;
    }

    pub async fn device_auth(&self, device_id: &str) -> Result<AuthResponse, String> {
        let resp = self
            .http
            .post(format!("{}/auth/device", self.api_base))
            .json(&DeviceAuthPayload {
                device_id: device_id.to_string(),
            })
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Device auth failed ({status}): {body}"));
        }

        resp.json::<AuthResponse>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    }

    pub async fn upload_session(
        &self,
        session: &SessionUpload,
    ) -> Result<SessionUploadResponse, String> {
        let token = self.token.as_ref().ok_or("Not logged in")?;
        let resp = self
            .http
            .post(format!("{}/sessions", self.api_base))
            .bearer_auth(token)
            .json(session)
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Upload failed ({status}): {body}"));
        }

        resp.json::<SessionUploadResponse>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    }

    pub async fn get_me(&self) -> Result<MeResponse, String> {
        let token = self.token.as_ref().ok_or("Not logged in")?;
        let resp = self
            .http
            .get(format!("{}/me", self.api_base))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Failed to fetch profile ({status}): {body}"));
        }

        resp.json::<MeResponse>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    }

    pub async fn get_leaderboard(&self, limit: i64) -> Result<Vec<LeaderboardEntry>, String> {
        let resp = self
            .http
            .get(format!("{}/leaderboard", self.api_base))
            .query(&[("limit", limit)])
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Failed to fetch leaderboard ({status}): {body}"));
        }

        resp.json::<Vec<LeaderboardEntry>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    }
}
