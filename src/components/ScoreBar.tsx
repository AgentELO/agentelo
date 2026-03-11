interface ScoreBarProps {
  label: string;
  value: number;
  weight?: string;
}

export function ScoreBar({ label, value, weight }: ScoreBarProps) {
  const color =
    value >= 70 ? "bg-success" : value >= 40 ? "bg-warning" : "bg-danger";
  const width = Math.max(2, value);

  return (
    <div className="flex items-center gap-3 py-1.5">
      <span className="w-44 text-sm text-text-secondary shrink-0">{label}</span>
      <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full ${color} transition-all duration-500`}
          style={{ width: `${width}%` }}
        />
      </div>
      <span className="w-10 text-right font-mono text-sm font-semibold">
        {value}
      </span>
      {weight && (
        <span className="w-10 text-right text-xs text-text-muted">{weight}</span>
      )}
    </div>
  );
}
