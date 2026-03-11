# AgentELO

**Track how AI-natively you work. What's your AgentELO?**

AgentELO is a desktop app that observes your workflow and scores how effectively you use AI tools. It records window focus, keystrokes, file changes, terminal commands, and periodic screenshots, then uses Gemini 2.5 Flash to analyze your session across 5 behavioral dimensions:

- **Delegation (30%)** - Did you make AI do the work?
- **Iteration (20%)** - Did you refine with AI, not one-shot?
- **Parallelism (20%)** - Multiple AI workstreams at once?
- **Independence (15%)** - Avoided manual fallbacks?
- **Shipping (15%)** - Did you produce output?

Your sessions contribute to an ELO rating (starting at 1200) that tracks your AI-native growth over time.

## Architecture

- **Desktop app**: Tauri v2 (Rust) + React + TypeScript + Tailwind
- **Local storage**: SQLite at `~/.agentelo/agentelo.db`
- **AI scoring**: Gemini 2.5 Flash via hosted backend (multimodal: screenshots + events)
- **Auth**: Anonymous device auth on first launch, optional Google sign-in to claim account

All your data is stored locally. The hosted backend at `agentelo.ai` handles AI scoring and the global leaderboard.

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v20+)
- macOS (primary platform — Linux/Windows support planned)

### Setup

```bash
npm install
npx @tauri-apps/cli dev
```

### Build

```bash
npx @tauri-apps/cli build
```

The built app will be at `src-tauri/target/release/bundle/macos/AgentELO.app`.

## Project Structure

```
agentelo/
  src/                  # React frontend
    components/         # UI components
    lib/                # API client, types
  src-tauri/
    src/
      capture/          # 6 capture modules (window, keystrokes, filesystem, terminal, screenshot, clipboard)
      analysis/         # ELO calculation
      lib.rs            # Tauri commands, recording lifecycle, tray, auto-sync
      db.rs             # SQLite operations
      sync.rs           # Cloud API client
      auth.rs           # Device auth + Google OAuth
```

## How It Works

1. **Start recording** - Click the tray icon or dashboard button
2. **Work normally** - AgentELO captures your workflow in the background
3. **Stop recording** - Your session is analyzed by Gemini 2.5 Flash
4. **Get scored** - See your dimension scores, ELO change, and AI coaching insights

## macOS Permissions

AgentELO requires these System Settings permissions:
- **Accessibility** - For keystroke velocity tracking (counts only, not content)
- **Screen Recording** - For periodic screenshots during sessions

## Privacy

AgentELO captures workflow data while recording is active. Here's exactly what:

- **Window titles** - Active app name and window title (browser URLs are stripped to titles only)
- **Keystroke velocity** - Keys-per-minute count and typing pattern (steady, burst, etc.). No key content is recorded.
- **File changes** - Git repo names, changed file count, insertions/deletions
- **Terminal commands** - Command names with sensitive arguments redacted (tokens, passwords, API keys are stripped)
- **Screenshots** - Full-screen capture every 5 seconds, stored locally as JPEG
- **Clipboard** - Only metadata (content length, whether it looks like code). No clipboard content is stored.

**What gets sent to the server:** When a session ends, a sample of screenshots (up to 20, resized to 512px) and an events summary are uploaded to `agentelo.ai`, which forwards them to Google Gemini 2.5 Flash for analysis. Screenshots and events are not persisted on the server after scoring.

**Local storage:** All data is stored in `~/.agentelo/agentelo.db` (SQLite) and `~/.agentelo/frames/` (screenshots). To delete all local data, remove the `~/.agentelo` directory.

## License

[MIT](LICENSE)
