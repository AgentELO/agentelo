use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use uuid::Uuid;

const AUTH_TIMEOUT_SECS: u64 = 300;

#[derive(Serialize, Deserialize, Clone)]
pub struct StoredAuth {
    pub token: String,
    pub user: crate::sync::UserInfo,
}

fn agentelo_dir() -> PathBuf {
    let dir = dirs::home_dir().unwrap().join(".agentelo");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn auth_file_path() -> PathBuf {
    agentelo_dir().join("auth.json")
}

fn device_file_path() -> PathBuf {
    agentelo_dir().join("device.json")
}

/// Get or create a persistent device ID
pub fn get_device_id() -> String {
    #[derive(Serialize, Deserialize)]
    struct DeviceFile {
        device_id: String,
    }

    if let Ok(data) = std::fs::read_to_string(device_file_path()) {
        if let Ok(file) = serde_json::from_str::<DeviceFile>(&data) {
            return file.device_id;
        }
    }

    let id = Uuid::new_v4().to_string();
    let file = DeviceFile {
        device_id: id.clone(),
    };
    if let Ok(data) = serde_json::to_string_pretty(&file) {
        let path = device_file_path();
        let _ = std::fs::write(&path, data);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
    id
}

pub fn load_stored_auth() -> Option<StoredAuth> {
    let data = std::fs::read_to_string(auth_file_path()).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_auth(auth: &StoredAuth) {
    if let Ok(data) = serde_json::to_string_pretty(auth) {
        let path = auth_file_path();
        let _ = std::fs::write(&path, &data);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

pub fn clear_auth() {
    let _ = std::fs::remove_file(auth_file_path());
}

/// Run the full OAuth flow:
/// 1. Bind a local TCP server on a random port
/// 2. Open system browser to backend's desktop auth start endpoint
/// 3. Backend handles Google OAuth and redirects back to our local server with a JWT
/// 4. Validate the token by calling /me
/// 5. Persist token + user to disk
pub async fn run_oauth_flow(api_base: &str) -> Result<StoredAuth, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed to bind local auth server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();

    let auth_url = format!("{api_base}/auth/desktop/start?port={port}");

    std::process::Command::new("open")
        .arg(&auth_url)
        .spawn()
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    let token = tokio::time::timeout(
        std::time::Duration::from_secs(AUTH_TIMEOUT_SECS),
        wait_for_callback(listener),
    )
    .await
    .map_err(|_| "Login timed out (5 minutes). Please try again.".to_string())??;

    // Validate token by fetching user profile
    let client = crate::sync::SyncClient::with_token(api_base, &token);
    let me = client.get_me().await?;

    let stored = StoredAuth {
        token,
        user: me.user,
    };
    save_auth(&stored);
    Ok(stored)
}

async fn wait_for_callback(listener: TcpListener) -> Result<String, String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| format!("Failed to accept callback: {e}"))?;

    let mut buf = vec![0u8; 8192];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Failed to read callback: {e}"))?;

    let request = String::from_utf8_lossy(&buf[..n]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("");

    if let Some(error) = extract_query_param(path, "error") {
        let html = format!(
            "<html><body style='font-family:system-ui;text-align:center;padding:60px;background:#0a0a0f;color:#e5e5e5'>\
             <h2>Login failed</h2><p style='color:#ef4444'>{error}</p>\
             <p style='color:#666'>You can close this tab.</p></body></html>"
        );
        send_response(&mut stream, &html).await;
        return Err(format!("Login failed: {error}"));
    }

    let token = extract_query_param(path, "token")
        .ok_or_else(|| "No token in callback".to_string())?;

    let html = "<html><body style='font-family:system-ui;text-align:center;padding:60px;background:#0a0a0f;color:#e5e5e5'>\
        <h2 style='color:#818cf8'>Login successful!</h2>\
        <p style='color:#666'>Returning to AgentELO...</p>\
        <script>setTimeout(function(){window.close()},500)</script>\
        </body></html>";
    send_response(&mut stream, html).await;

    Ok(token)
}

async fn send_response(stream: &mut tokio::net::TcpStream, html: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n{html}"
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}

fn extract_query_param(path: &str, key: &str) -> Option<String> {
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let k = kv.next()?;
        let v = kv.next()?;
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}
