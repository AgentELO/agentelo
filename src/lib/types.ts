export interface CurrentStatus {
  recording: boolean;
  session_id: number | null;
  current_elo: number;
  percentile: number | null;
  total_sessions: number;
  total_badges: number;
}

export interface SessionSummary {
  id: number;
  started_at: string;
  stopped_at: string | null;
  elo_before: number;
  elo_after: number | null;
  percentile: number | null;
  overall_score: number | null;
}

export interface SessionDetail {
  session: SessionSummary;
  score_json: string | null;
  insights_json: string | null;
  gemini_score_json: string | null;
  event_count: number;
}

export interface GeminiScores {
  delegation: number;
  iteration: number;
  parallelism: number;
  independence: number;
  shipping: number;
  overall: number;
}


export interface Badge {
  id: number;
  session_id: number | null;
  badge_name: string;
  earned_at: string;
}

export interface UserInfo {
  id: number;
  google_id: string | null;
  email: string | null;
  name: string;
  avatar_url: string | null;
  current_elo: number;
  total_sessions: number;
}

export interface LeaderboardEntry {
  rank: number;
  name: string;
  avatar_url: string | null;
  elo: number;
  total_sessions: number;
  trend: number;
}

export const BADGE_INFO: Record<string, { name: string; icon: string; description: string }> = {
  first_session: { name: "First Session", icon: "1", description: "Completed your first tracked session" },
  ai_native: { name: "AI Native", icon: "AN", description: "Scored above 80 in a session" },
  multi_tool: { name: "Multi-Tool", icon: "MT", description: "Used 3+ AI tools in one session" },
  speed_demon: { name: "Speed Demon", icon: "SD", description: "500+ lines of code changes" },
  prompt_engineer: { name: "Prompt Engineer", icon: "PE", description: "High prompt effectiveness" },
  zero_manual: { name: "Zero Manual", icon: "ZM", description: "No manual patterns detected" },
  rising_star: { name: "Rising Star", icon: "RS", description: "ELO increased 50+ points" },
  manual_mode: { name: "Manual Mode", icon: "MM", description: "Score below 30" },
};

export const CATEGORY_LABELS: Record<string, string> = {
  delegation: "Delegation",
  iteration: "Iteration",
  parallelism: "Parallelism",
  independence: "Independence",
  shipping: "Shipping",
};
