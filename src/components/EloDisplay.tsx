interface EloDisplayProps {
  elo: number;
  percentile: number | null;
  eloBefore?: number;
}

export function EloDisplay({ elo, percentile, eloBefore }: EloDisplayProps) {
  const change = eloBefore ? elo - eloBefore : 0;

  return (
    <div className="space-y-4">
      <div className="flex items-baseline gap-4">
        <div>
          <span className="text-text-secondary text-sm">Your AgentELO</span>
          <div className="flex items-baseline gap-2">
            <span className="text-4xl font-bold font-mono bg-gradient-to-r from-brand to-accent bg-clip-text text-transparent">
              {Math.round(elo).toLocaleString()}
            </span>
            {change !== 0 && (
              <span
                className={`text-sm font-mono ${
                  change > 0 ? "text-success" : "text-danger"
                }`}
              >
                {change > 0 ? "+" : ""}
                {Math.round(change)}
              </span>
            )}
          </div>
        </div>
        {percentile != null && (
          <div className="ml-auto text-right">
            <span className="text-text-secondary text-sm">Percentile</span>
            <div className="text-2xl font-bold font-mono text-brand-light">
              Top {(100 - percentile).toFixed(1)}%
            </div>
          </div>
        )}
      </div>

      {percentile != null && (
        <>
          <div className="h-2 bg-white/5 rounded-full overflow-hidden">
            <div
              className="h-full rounded-full bg-gradient-to-r from-brand to-accent transition-all duration-700"
              style={{ width: `${Math.min(98, percentile)}%` }}
            />
          </div>
          <div className="text-xs text-text-muted text-center">
            {percentile.toFixed(1)}th percentile
          </div>
        </>
      )}
    </div>
  );
}
