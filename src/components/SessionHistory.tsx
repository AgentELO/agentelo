import type { SessionSummary } from "../lib/types";

interface SessionHistoryProps {
  sessions: SessionSummary[];
  onSelect: (id: number) => void;
  selectedId?: number;
}

export function SessionHistory({ sessions, onSelect, selectedId }: SessionHistoryProps) {
  const completed = sessions.filter((s) => s.stopped_at && s.elo_after != null);

  if (!completed.length) {
    return <p className="text-text-muted text-sm">No sessions yet.</p>;
  }

  return (
    <div className="space-y-1">
      {completed.map((s) => {
        const change = (s.elo_after ?? 0) - s.elo_before;
        const date = s.started_at.slice(0, 16).replace("T", " ");
        const isSelected = s.id === selectedId;

        return (
          <button
            key={s.id}
            onClick={() => onSelect(s.id)}
            className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left transition-colors ${
              isSelected
                ? "bg-brand/10 border border-brand/30"
                : "hover:bg-surface-hover border border-transparent"
            }`}
          >
            <span className="text-xs text-text-muted font-mono w-6">#{s.id}</span>
            <span className="text-sm text-text-secondary flex-1">{date}</span>
            <span className="text-sm font-mono font-semibold">
              {Math.round(s.elo_after ?? 0)}
            </span>
            <span
              className={`text-xs font-mono w-12 text-right ${
                change >= 0 ? "text-success" : "text-danger"
              }`}
            >
              {change >= 0 ? "+" : ""}
              {Math.round(change)}
            </span>
            {s.overall_score != null && (
              <span className="text-xs text-text-muted w-10 text-right">
                {s.overall_score.toFixed(0)}/100
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
