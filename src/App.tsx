import { useEffect, useRef, useState } from "react";
import "./App.css";
import {
  getStatus, getSessions, getSessionDetail, getBadges,
  startRecording, stopRecording, login, logout, getAuthState,
  checkForUpdate,
} from "./lib/api";
import type { UpdateInfo } from "./lib/api";
import type { CurrentStatus, SessionSummary, SessionDetail, Badge, UserInfo } from "./lib/types";
import { EloDisplay } from "./components/EloDisplay";
import { SessionHistory } from "./components/SessionHistory";
import { SessionReport } from "./components/SessionReport";
import { BadgeGrid } from "./components/BadgeGrid";
import { Leaderboard } from "./components/Leaderboard";
import { Settings } from "./components/Settings";

type Tab = "overview" | "history" | "badges" | "leaderboard" | "settings";

function App() {
  const [user, setUser] = useState<UserInfo | null>(null);
  const [status, setStatus] = useState<CurrentStatus | null>(null);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [badges, setBadges] = useState<Badge[]>([]);
  const [selectedSession, setSelectedSession] = useState<SessionDetail | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [error, setError] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const insightsPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Clean up insights polling on unmount
  useEffect(() => {
    return () => {
      if (insightsPollRef.current) clearInterval(insightsPollRef.current);
    };
  }, []);

  // Check for updates on mount
  useEffect(() => {
    checkForUpdate()
      .then((info) => {
        if (info.available) setUpdateInfo(info);
      })
      .catch(() => {}); // Silently fail
  }, []);

  // Poll auth state (device auth happens silently in Rust on startup)
  useEffect(() => {
    const checkAuth = () => {
      getAuthState().then((u) => setUser(u)).catch(() => {});
    };
    checkAuth();
    const interval = setInterval(checkAuth, 10000);
    return () => clearInterval(interval);
  }, []);

  // Poll local data always (app works without auth)
  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, []);

  async function loadData() {
    try {
      const [s, sess, b] = await Promise.all([
        getStatus(),
        getSessions(),
        getBadges(),
      ]);
      setStatus(s);
      setSessions(sess);
      setBadges(b);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleClaimAccount() {
    try {
      const u = await login(); // Opens browser for Google OAuth to claim account
      setUser(u);
    } catch {
      // User cancelled or failed — ignore, they stay anonymous
    }
  }

  async function handleLogout() {
    try {
      await logout();
    } catch {
      // Ignore logout errors
    }
    setUser(null);
  }

  async function handleSelectSession(id: number) {
    setSelectedId(id);
    try {
      const detail = await getSessionDetail(id);
      setSelectedSession(detail);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    if (sessions.length && !selectedId) {
      const latest = sessions.find((s) => s.stopped_at && s.elo_after != null);
      if (latest) handleSelectSession(latest.id);
    }
  }, [sessions]);

  return (
    <div className="min-h-screen bg-surface">
      <header className="sticky top-0 z-10 bg-surface/80 backdrop-blur-xl border-b border-border px-6 py-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-brand to-accent flex items-center justify-center text-xs font-bold font-mono">
              E
            </div>
            <span className="font-semibold text-lg">
              Agent<span className="text-brand-light">ELO</span>
            </span>
          </div>
          <div className="flex items-center gap-3">
            <button
              disabled={syncing}
              onClick={async () => {
                try {
                  if (status?.recording) {
                    const result = await stopRecording();
                    await loadData();
                    // Extract session id from result message
                    const match = result.match(/session (\d+)/);
                    const stoppedId = match ? parseInt(match[1]) : null;
                    if (stoppedId) {
                      setTab("overview");
                      await handleSelectSession(stoppedId);
                      // Poll for insights (background sync takes a few seconds)
                      if (insightsPollRef.current) clearInterval(insightsPollRef.current);
                      setSyncing(true);
                      let attempts = 0;
                      insightsPollRef.current = setInterval(async () => {
                        attempts++;
                        try {
                          const detail = await getSessionDetail(stoppedId);
                          setSelectedSession(detail);
                          if (detail.gemini_score_json || detail.insights_json || attempts >= 30) {
                            if (insightsPollRef.current) clearInterval(insightsPollRef.current);
                            insightsPollRef.current = null;
                            setSyncing(false);
                          }
                        } catch {
                          if (insightsPollRef.current) clearInterval(insightsPollRef.current);
                          insightsPollRef.current = null;
                          setSyncing(false);
                        }
                      }, 2000);
                    }
                  } else {
                    await startRecording();
                    await loadData();
                  }
                } catch (e) {
                  setError(String(e));
                  setSyncing(false);
                }
              }}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-colors ${
                syncing
                  ? "bg-brand/10 border border-brand/20 text-brand-light"
                  : status?.recording
                  ? "bg-danger/10 border border-danger/20 text-danger hover:bg-danger/20"
                  : "bg-success/10 border border-success/20 text-success hover:bg-success/20"
              }`}
            >
              <div className={`w-2 h-2 rounded-full ${syncing ? "bg-brand-light animate-pulse" : status?.recording ? "bg-danger animate-pulse" : "bg-success"}`} />
              {syncing ? "Generating Insights..." : status?.recording ? "Stop Recording" : "Start Recording"}
            </button>
            {status && (
              <span className="font-mono text-sm font-semibold text-brand-light">
                {Math.round(status.current_elo).toLocaleString()}
              </span>
            )}
          </div>
        </div>
        <nav className="flex gap-1 mt-3 -mb-3">
          {(["overview", "history", "badges", "leaderboard", "settings"] as Tab[]).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`px-3 py-2 text-sm rounded-t-lg transition-colors ${
                tab === t
                  ? "text-text-primary bg-surface-raised border-b-2 border-brand"
                  : "text-text-muted hover:text-text-secondary"
              }`}
            >
              {t.charAt(0).toUpperCase() + t.slice(1)}
            </button>
          ))}
        </nav>
      </header>

      {updateInfo && (
        <div className="mx-6 mt-4 px-4 py-2 rounded-lg bg-brand/10 border border-brand/20 text-sm flex items-center justify-between">
          <span className="text-brand-light">
            AgentELO v{updateInfo.version} is available
          </span>
          <a
            href={updateInfo.download_url ?? "https://agentelo.ai"}
            target="_blank"
            rel="noopener"
            className="text-xs font-medium px-3 py-1 rounded-md bg-brand text-[#09090b] hover:bg-brand-light transition-colors"
          >
            Download Update
          </a>
        </div>
      )}

      {error && (
        <div className="mx-6 mt-4 px-4 py-2 rounded-lg bg-danger/10 border border-danger/20 text-danger text-sm">
          {error}
        </div>
      )}

      <main className="p-6 max-w-4xl mx-auto">
        {tab === "overview" && status && (
          <div className="space-y-6">
            <div className="bg-surface-raised rounded-xl border border-border p-6">
              <EloDisplay elo={status.current_elo} percentile={status.percentile} />
            </div>
            <div className="grid grid-cols-3 gap-4">
              <QuickStat label="Sessions" value={status.total_sessions} />
              <QuickStat label="Badges" value={status.total_badges} />
              <QuickStat
                label="Status"
                value={status.recording ? "Recording" : "Idle"}
                color={status.recording ? "text-success" : "text-text-muted"}
              />
            </div>
            {selectedSession && (
              <div>
                <h2 className="text-sm font-medium text-text-secondary mb-3">Latest Session Report</h2>
                <SessionReport detail={selectedSession} />
                {syncing && !selectedSession.insights_json && (
                  <div className="mt-4 flex items-center gap-2 text-sm text-brand-light">
                    <div className="w-3 h-3 rounded-full bg-brand-light animate-pulse" />
                    Analyzing your session with AI... This takes a few seconds.
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {tab === "history" && (
          <div className="grid grid-cols-[280px_1fr] gap-6">
            <div>
              <h2 className="text-sm font-medium text-text-secondary mb-3">Sessions</h2>
              <SessionHistory
                sessions={sessions}
                onSelect={handleSelectSession}
                selectedId={selectedId ?? undefined}
              />
            </div>
            <div>
              {selectedSession ? (
                <SessionReport detail={selectedSession} />
              ) : (
                <p className="text-text-muted text-sm">Select a session to view details.</p>
              )}
            </div>
          </div>
        )}

        {tab === "badges" && (
          <div>
            <h2 className="text-sm font-medium text-text-secondary mb-4">
              Badge Collection ({badges.length})
            </h2>
            <BadgeGrid badges={badges} />
          </div>
        )}

        {tab === "leaderboard" && (
          <div>
            <h2 className="text-sm font-medium text-text-secondary mb-4">Leaderboard</h2>
            <Leaderboard />
          </div>
        )}

        {tab === "settings" && (
          <Settings user={user} onLogin={handleClaimAccount} onLogout={handleLogout} />
        )}
      </main>
    </div>
  );
}

function QuickStat({ label, value, color }: { label: string; value: string | number; color?: string }) {
  return (
    <div className="bg-surface-raised rounded-xl border border-border p-4">
      <div className="text-xs text-text-muted">{label}</div>
      <div className={`text-xl font-mono font-bold mt-1 ${color ?? ""}`}>{String(value)}</div>
    </div>
  );
}

export default App;
