import { useEffect, useState } from "react";
import { getLeaderboardData } from "../lib/api";
import type { LeaderboardEntry } from "../lib/types";

export function Leaderboard() {
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getLeaderboardData()
      .then(setEntries)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <p className="text-text-muted text-sm">Loading leaderboard...</p>;
  if (error) return <p className="text-danger text-sm">{error}</p>;
  if (!entries.length) return <p className="text-text-muted text-sm">No entries yet. Be the first!</p>;

  return (
    <div className="space-y-2">
      {entries.map((e) => (
        <div
          key={e.rank}
          className="flex items-center gap-3 bg-surface-raised rounded-lg border border-border px-4 py-3"
        >
          <span className="w-8 text-right font-mono text-sm text-text-muted">
            #{e.rank}
          </span>
          {e.avatar_url ? (
            <img src={e.avatar_url} className="w-8 h-8 rounded-full" alt="" />
          ) : (
            <div className="w-8 h-8 rounded-full bg-brand/20 flex items-center justify-center text-xs font-bold">
              {e.name.charAt(0)}
            </div>
          )}
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium truncate">{e.name}</div>
            <div className="text-xs text-text-muted">{e.total_sessions} sessions</div>
          </div>
          <div className="text-right">
            <div className="font-mono font-bold text-brand-light">
              {Math.round(e.elo)}
            </div>
            {e.trend !== 0 && (
              <div className={`text-xs font-mono ${e.trend > 0 ? "text-success" : "text-danger"}`}>
                {e.trend > 0 ? "+" : ""}{e.trend.toFixed(0)}
              </div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
