# VibeIsland Linux — Roadmap

Plan de développement jusqu'à la release v0.1 publique (AppImage).

Voir aussi : [brief](./brief.md) · [research](./research.md) · [architecture](./architecture.md).

## Légende

- `auto` = label `auto-execute` — confiance haute, Autopilot peut dispatcher
- `human` = label `needs-human` — décision / recherche / review humaine requise
- ✅ = issue fermée · ⏳ = issue ouverte

---

## Phase 0 — Project scaffolding, Tauri setup, CI (1-2 jours)

- ⏳ [#1 Init Tauri v2 project avec React + TypeScript + Tailwind](../../../issues/1) — `auto` `agent:devops`
- ⏳ [#2 Configurer Tauri pour overlay flottant (decorations, always-on-top, transparent)](../../../issues/2) — `auto` `agent:devops`
- ⏳ [#3 Setup Cargo workspace avec crates internes (agents, terminal, sound)](../../../issues/3) — `auto` `agent:devops`
- ⏳ [#4 GitHub Actions CI : lint Rust + TypeScript + tests](../../../issues/4) — `auto` `agent:devops`
- ⏳ [#5 README et CONTRIBUTING.md](../../../issues/5) — `auto` `agent:devops`
- ⏳ [#6 License MIT](../../../issues/6) — `auto` `agent:devops`
- ⏳ [#7 Pre-commit hooks (rustfmt, clippy, prettier, eslint)](../../../issues/7) — `auto` `agent:devops`

## Phase 1 — MVP Claude Code + overlay + approve/deny (1 semaine)

- ⏳ [#8 Trait Agent Rust (contrat generique pour tous les adapters)](../../../issues/8) — `auto` `agent:feature`
- ⏳ [#9 Adapter Claude Code : install/uninstall hooks dans ~/.claude/settings.json](../../../issues/9) — `human` `agent:feature`
- ⏳ [#10 CLI subcommand `vibeisland-linux hook <event>` (capture hooks Claude Code)](../../../issues/10) — `auto` `agent:feature`
- ⏳ [#11 File watcher `~/.vibeisland/events/` avec crate `notify`](../../../issues/11) — `auto` `agent:feature`
- ⏳ [#12 Session state store `~/.vibeisland/sessions.json` (read/write atomique)](../../../issues/12) — `auto` `agent:feature`
- ⏳ [#13 Tauri IPC commands (list_sessions, approve, deny, answer_question, focus_terminal)](../../../issues/13) — `auto` `agent:feature`
- ⏳ [#14 Mécanisme approve/deny : débloquer Claude Code après clic utilisateur](../../../issues/14) — `human` `agent:feature`
- ⏳ [#15 Frontend: composant OverlayPanel (layout principal, draggable)](../../../issues/15) — `auto` `agent:feature`
- ⏳ [#16 Frontend: composant AgentCard avec états visuels (idle/thinking/awaiting)](../../../issues/16) — `auto` `agent:feature`
- ⏳ [#17 Frontend: composant ApprovalPrompt (UI approve/deny)](../../../issues/17) — `auto` `agent:feature`
- ⏳ [#18 Frontend: hook React `useAgentState` (subscribe aux events Tauri)](../../../issues/18) — `auto` `agent:feature`
- ⏳ [#19 Types partagés TS ↔ Rust (AgentSession, PendingAction, SessionState)](../../../issues/19) — `auto` `agent:feature`
- ⏳ [#20 Smoke test E2E : hook Claude Code → session visible dans UI](../../../issues/20) — `auto` `agent:feature`

## Phase 2 — Terminal nav + AskUserQuestion + sons (1 semaine)

- ⏳ [#21 Trait Rust `TerminalLocator` (contrat pour tous les émulateurs)](../../../issues/21) — `auto` `agent:feature`
- ⏳ [#22 TerminalLocator: GNOME Terminal (wmctrl + xdotool)](../../../issues/22) — `auto` `agent:feature`
- ⏳ [#23 TerminalLocator: Konsole (D-Bus)](../../../issues/23) — `auto` `agent:feature`
- ⏳ [#24 TerminalLocator: Kitty (socket remote control)](../../../issues/24) — `auto` `agent:feature`
- ⏳ [#25 TerminalLocator: Alacritty (fallback wmctrl)](../../../issues/25) — `auto` `agent:feature`
- ⏳ [#26 Capturer terminal info dans le hook (emulator, window_id, tab_id, pid)](../../../issues/26) — `human` `agent:feature`
- ⏳ [#27 Frontend: bouton « Focus terminal » actif dans AgentCard](../../../issues/27) — `auto` `agent:feature`
- ⏳ [#28 Détection + parsing `AskUserQuestion` via hook PreToolUse](../../../issues/28) — `auto` `agent:feature`
- ⏳ [#29 Frontend: composant AskQuestion (UI multi-choix)](../../../issues/29) — `auto` `agent:feature`
- ⏳ [#30 Sound player Rust avec crate `rodio`](../../../issues/30) — `auto` `agent:feature`
- ⏳ [#31 Sound samples 8-bit (alert, approval, question, done)](../../../issues/31) — `human` `agent:designer`
- ⏳ [#32 Config user `~/.config/vibeisland-linux/config.json`](../../../issues/32) — `auto` `agent:feature`
- ⏳ [#33 Frontend: écran Settings (volume, mute, agents, theme)](../../../issues/33) — `auto` `agent:feature`
- ⏳ [#34 Pill collapsed mode (réduction overlay en pastille)](../../../issues/34) — `auto` `agent:feature`

## Phase 3 — AppImage release v0.1 (2-3 jours)

- ⏳ [#35 GitHub Actions: workflow release (build AppImage sur tag)](../../../issues/35) — `auto` `agent:devops`
- ⏳ [#36 Icônes de l'app (multi-tailles) + fichier .desktop](../../../issues/36) — `human` `agent:designer`
- ⏳ [#37 Tauri config pour AppImage bundle (tauri.conf.json)](../../../issues/37) — `auto` `agent:devops`
- ⏳ [#38 Test de l'AppImage v0.1 sur Ubuntu LTS, Fedora, Arch](../../../issues/38) — `human` `agent:devops`
- ⏳ [#39 CHANGELOG.md + release notes v0.1](../../../issues/39) — `auto` `agent:devops`
- ⏳ [#40 Release v0.1.0: tag, publication, announcement](../../../issues/40) — `human` `agent:devops`

---

## Statistiques MVP (phases 0-3)

- **Total** : 40 issues
- **auto-execute** : 33 (Autopilot peut dispatcher)
- **needs-human** : 7 (review/décision humaine)

| Phase | Total | auto | human |
|-------|-------|------|-------|
| 0 | 7 | 7 | 0 |
| 1 | 13 | 11 | 2 |
| 2 | 14 | 12 | 2 |
| 3 | 6 | 3 | 3 |

---

## Phases suivantes (hors MVP)

- **Phase 4** — Codex + Gemini CLI + Cursor (2 semaines)
- **Phase 5** — OpenCode / Droid / Qoder / Copilot / CodeBuddy / Kiro + tray + Flatpak (3 semaines)
- **Phase 6** — v1.0 feature parity avec VibeIsland macOS

_Généré automatiquement depuis les issues GitHub. Source de vérité : la liste d'issues, ce fichier est un index._