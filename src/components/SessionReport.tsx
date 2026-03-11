import type { SessionDetail, GeminiScores } from "../lib/types";
import { CATEGORY_LABELS } from "../lib/types";
import { ScoreBar } from "./ScoreBar";
import { EloDisplay } from "./EloDisplay";

interface SessionReportProps {
  detail: SessionDetail;
}

const WEIGHTS: Record<string, string> = {
  delegation: "30%",
  iteration: "20%",
  parallelism: "20%",
  independence: "15%",
  shipping: "15%",
};

export function SessionReport({ detail }: SessionReportProps) {
  const { session } = detail;

  const geminiScores: GeminiScores | null = (() => {
    if (!detail.gemini_score_json) return null;
    try {
      return JSON.parse(detail.gemini_score_json);
    } catch {
      return null;
    }
  })();

  const insightsText = (() => {
    if (!detail.insights_json) return null;
    try {
      const parsed = JSON.parse(detail.insights_json);
      if (parsed.insights) return parsed.insights;
      if (parsed.analysis) return parsed.analysis;
      return JSON.stringify(parsed, null, 2);
    } catch {
      return detail.insights_json;
    }
  })();

  if (!geminiScores) {
    return <p className="text-text-muted">No score data for this session.</p>;
  }

  const categories: Record<string, number> = {
    delegation: geminiScores.delegation,
    iteration: geminiScores.iteration,
    parallelism: geminiScores.parallelism,
    independence: geminiScores.independence,
    shipping: geminiScores.shipping,
  };
  const overall = geminiScores.overall;

  return (
    <div className="space-y-6">
      <EloDisplay
        elo={session.elo_after ?? session.elo_before}
        percentile={session.percentile ?? null}
        eloBefore={session.elo_before}
      />

      <div className="bg-surface-raised rounded-xl border border-border p-4 space-y-1">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-medium text-text-secondary">
            Score Breakdown
          </h3>
          <span className="text-xs text-brand px-2 py-0.5 rounded-full bg-brand/10 border border-brand/20">
            AI Verified
          </span>
        </div>
        {Object.entries(categories).map(([key, value]) => (
          <ScoreBar
            key={key}
            label={CATEGORY_LABELS[key] ?? key}
            value={value}
            weight={WEIGHTS[key]}
          />
        ))}
        <div className="pt-2 border-t border-border mt-2 flex items-center justify-between">
          <span className="text-sm font-medium">Overall</span>
          <span
            className={`text-lg font-bold font-mono ${
              overall >= 70
                ? "text-success"
                : overall >= 40
                ? "text-warning"
                : "text-danger"
            }`}
          >
            {overall.toFixed(1)}/100
          </span>
        </div>
      </div>

      {insightsText && (
        <div className="bg-surface-raised rounded-xl border border-brand/20 p-4">
          <h3 className="text-sm font-medium text-brand-light mb-3">
            AI Insights
          </h3>
          <div className="text-sm text-text-secondary whitespace-pre-wrap leading-relaxed">
            {insightsText}
          </div>
        </div>
      )}
    </div>
  );
}

