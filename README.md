# VibeIsland Linux

> Supervise your AI coding agents from a floating overlay on Linux.

Inspired by [VibeIsland](https://vibeisland.app/) (macOS only) — this is a Linux-native reimplementation, because we also want to stop context-switching between terminals.

## Status

Early development (Phase 0). See [docs/brief.md](docs/brief.md) for vision, [docs/research.md](docs/research.md) for competitive analysis and [docs/architecture.md](docs/architecture.md) for technical details. Track progress in [docs/roadmap.md](docs/roadmap.md).

## Goals

- Supervise Claude Code, Codex, Gemini CLI, Cursor, Copilot and more from a single overlay
- Approve / deny tool permissions with a click
- Answer `AskUserQuestion` prompts without switching to the terminal
- Jump to the exact terminal tab hosting a session
- Cross-DE: works on GNOME, KDE, Hyprland, Sway, i3
- Open-source, MIT licensed

## Stack

- Tauri v2 (Rust + React + TypeScript + Tailwind v4)
- Cargo workspace — binary `src-tauri` plus `crates/{agents,terminal,sound}`
- AppImage packaging at v0.1 (Flatpak / .deb / .rpm planned)
- Cross-distro, Wayland + X11

## Install (users)

Not yet released. A v0.1 AppImage will land when Phase 3 is done — watch the [releases page](https://github.com/jayisamot/vibeisland-linux/releases).

## Develop (contributors)

### Prerequisites

- Linux (X11 or Wayland)
- Node 20+ and npm
- Rust stable (`rustup default stable`)
- Tauri Linux deps: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf` (see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#linux))

### Setup

```bash
git clone https://github.com/jayisamot/vibeisland-linux.git
cd vibeisland-linux
npm install
npm run tauri dev
```

### Project layout

```
vibeisland-linux/
├── Cargo.toml              # workspace root
├── src-tauri/              # Tauri binary (Rust)
├── crates/
│   ├── agents/             # Agent trait + adapters (Claude Code, ...)
│   ├── terminal/           # TerminalLocator adapters (kitty, konsole, ...)
│   └── sound/              # SoundPlayer (rodio)
├── src/                    # React frontend
│   ├── App.tsx
│   ├── components/         # OverlayPanel, AgentCard, ApprovalPrompt, ...
│   └── hooks/              # useAgentState, useConfig, ...
├── public/
└── docs/                   # brief, research, architecture, roadmap
```

### Scripts

- `npm run tauri dev` — launch the overlay with hot reload
- `npm run tauri build` — build the AppImage (release)
- `npm run build` — type-check + Vite build (frontend only)
- `cargo fmt` / `cargo clippy` — format / lint Rust
- `cargo test` — run Rust tests

## Roadmap

- [ ] **Phase 0** — Project scaffolding, Tauri setup, CI
- [ ] **Phase 1** — MVP: Claude Code supervision + floating overlay + approve/deny
- [ ] **Phase 2** — Terminal navigation + AskUserQuestion + sound alerts
- [ ] **Phase 3** — AppImage release v0.1
- [ ] **Phase 4** — Codex + Gemini CLI + Cursor
- [ ] **Phase 5** — Remaining agents + tray mode + Flatpak
- [ ] **Phase 6** — Feature parity with macOS original

Full breakdown in [docs/roadmap.md](docs/roadmap.md).

## Contributing

Issues and PRs welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the [docs/](docs/) before diving in.

## License

[MIT](LICENSE) © 2026 Jay Isamot
