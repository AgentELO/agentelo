import type { Badge } from "../lib/types";
import { BADGE_INFO } from "../lib/types";

interface BadgeGridProps {
  badges: Badge[];
}

export function BadgeGrid({ badges }: BadgeGridProps) {
  if (!badges.length) {
    return (
      <p className="text-text-muted text-sm">
        No badges yet. Start a session to earn your first!
      </p>
    );
  }

  // deduplicate by badge_name, keep latest
  const unique = new Map<string, Badge>();
  for (const b of badges) {
    if (!unique.has(b.badge_name)) unique.set(b.badge_name, b);
  }

  return (
    <div className="grid grid-cols-4 gap-3">
      {[...unique.values()].map((badge) => {
        const info = BADGE_INFO[badge.badge_name];
        if (!info) return null;
        return (
          <div
            key={badge.id}
            className="flex flex-col items-center gap-1.5 p-3 rounded-lg bg-surface-raised border border-border hover:bg-surface-hover transition-colors"
          >
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-brand to-accent flex items-center justify-center text-xs font-bold font-mono">
              {info.icon}
            </div>
            <span className="text-xs font-medium text-center">{info.name}</span>
            <span className="text-[10px] text-text-muted text-center leading-tight">
              {info.description}
            </span>
          </div>
        );
      })}
    </div>
  );
}
