import { invoke } from "@tauri-apps/api/core";
import type {
  CurrentStatus, SessionSummary, SessionDetail, Badge,
  UserInfo, LeaderboardEntry, ApiKeyConfigResponse,
} from "./types";

// Local data
export async function getStatus(): Promise<CurrentStatus> {
  return invoke("get_status");
}

export async function getSessions(): Promise<SessionSummary[]> {
  return invoke("get_sessions");
}

export async function getSessionDetail(sessionId: number): Promise<SessionDetail> {
  return invoke("get_session_detail", { sessionId });
}

export async function getBadges(): Promise<Badge[]> {
  return invoke("get_badges");
}

// Recording
export async function startRecording(): Promise<string> {
  return invoke("start_recording");
}

export async function stopRecording(): Promise<string> {
  return invoke("stop_recording");
}

// Auth (Rust handles OAuth flow + persistence)
export async function login(): Promise<UserInfo> {
  return invoke("login");
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

export async function getAuthState(): Promise<UserInfo | null> {
  return invoke("get_auth_state");
}

// BYOK — Gemini API key
export async function getApiKeyConfig(): Promise<ApiKeyConfigResponse | null> {
  return invoke("get_api_key_config");
}

export async function saveApiKeyConfig(apiKey: string): Promise<void> {
  return invoke("save_api_key_config", { apiKey });
}

export async function clearApiKeyConfig(): Promise<void> {
  return invoke("clear_api_key_config");
}

// Cloud sync
export async function getLeaderboardData(): Promise<LeaderboardEntry[]> {
  return invoke("get_leaderboard_data");
}

// Update check
export interface UpdateInfo {
  available: boolean;
  version: string | null;
  download_url: string | null;
}

export async function checkForUpdate(): Promise<UpdateInfo> {
  return invoke("check_for_update");
}
