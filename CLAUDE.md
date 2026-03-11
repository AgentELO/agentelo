# CLAUDE.md - AgentELO

## What is AgentELO?

Desktop app that tracks how AI-natively you work. Records your workflow, sends it to Gemini 2.5 Flash for scoring across 5 behavioral dimensions, assigns an ELO rating, and gives AI-powered coaching.

## Architecture

```
agentelo/
  src/              → React frontend (Vite + Tailwind)
  src-tauri/src/    → Tauri v2 Rust backend
    capture/        → 6 capture modules (window, keystrokes, filesystem, terminal, screenshot, clipboard)
    analysis/       → ELO calculation
    lib.rs          → Tauri commands, recording lifecycle, tray, auto-sync
    db.rs           → SQLite operations
    sync.rs         → Cloud API client
    auth.rs         → Device auth + Google OAuth
```

## Stack

- **Desktop**: Tauri v2 (Rust) + React + TypeScript + Tailwind
- **Local storage**: SQLite (per-user at ~/.agentelo/agentelo.db)
- **AI Insights**: Gemini 2.5 Flash via hosted backend (multimodal: text + screenshots)
- **Auth**: Anonymous device auth on first launch, optional Google OAuth to claim account

## Key Concepts

- **ELO**: Start at 1200, K-factor 32, benchmark opponent at 1200
- **5 dimensions**: Delegation (30%), Iteration (20%), Parallelism (20%), Independence (15%), Shipping (15%)
- **Scoring**: All scoring is server-side via Gemini — no local heuristics
- **Data**: All session data stored locally in SQLite. Backend used for AI scoring + leaderboard only.

## Development

```bash
npm install
npx @tauri-apps/cli dev    # Run in dev mode
npx @tauri-apps/cli build  # Production build
```

`cargo` lives at `~/.cargo/bin/cargo`. Tauri CLI via npx, not global install.

## Design

- Dark theme, brand color cyan #06d6a0
- Fonts: Inter (sans), JetBrains Mono (mono)
