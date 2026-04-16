# VibeIsland Linux

> Supervise your AI coding agents from a floating overlay on Linux.

Inspired by [VibeIsland](https://vibeisland.app/) (macOS only) — this is a Linux-native reimplementation, because we also want to stop context-switching between terminals.

## Status

Early development (Phase 0). See [docs/brief.md](docs/brief.md) for vision and [docs/architecture.md](docs/architecture.md) for technical details.

## Goals

- Supervise Claude Code, Codex, Gemini CLI, Cursor, Copilot and more from a single overlay
- Approve / deny tool permissions with a click
- Answer `AskUserQuestion` prompts without switching to the terminal
- Jump to the exact terminal tab hosting a session
- Cross-DE: works on GNOME, KDE, Hyprland, Sway, i3
- Open-source, MIT licensed

## Stack

- Tauri v2 (Rust + React + TypeScript)
- AppImage packaging (Flatpak / .deb / .rpm planned)
- Cross-distro, Wayland + X11

## Roadmap

- [ ] **Phase 0** — Project scaffolding, Tauri setup, CI
- [ ] **Phase 1** — MVP: Claude Code supervision + floating overlay + approve/deny
- [ ] **Phase 2** — Terminal navigation + AskUserQuestion + sound alerts
- [ ] **Phase 3** — AppImage release v0.1
- [ ] **Phase 4** — Codex + Gemini CLI + Cursor
- [ ] **Phase 5** — Remaining agents + tray mode + Flatpak
- [ ] **Phase 6** — Feature parity with macOS original

## Contributing

Issues and PRs welcome. See [docs/](docs/) for vision, research and architecture before diving in.

## License

MIT
