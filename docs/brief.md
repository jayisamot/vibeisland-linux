# VibeIsland Linux — Brief

## Vision

Un panneau de contrôle overlay pour Linux qui supervise plusieurs agents de codage IA en parallèle, inspiré de [VibeIsland](https://vibeisland.app/) (macOS only, utilise la Dynamic Island).

**Problème résolu** : les devs qui utilisent Claude Code, Codex, Cursor, Gemini CLI, etc. passent leur temps à basculer entre terminaux pour approuver des permissions, répondre à des AskUserQuestion, ou vérifier l'avancement. Sur macOS, VibeIsland centralise ça dans l'encoche. Sur Linux, rien n'existe.

## Différence clé avec l'original

VibeIsland macOS utilise la Dynamic Island (encoche du MacBook) comme point d'ancrage. Linux n'a pas d'équivalent natif. Solution : **overlay flottant draggable** (toujours-visible, positionnable, rétractable), avec option fallback **tray icon** pour les users qui préfèrent discret.

## Scope MVP (v0.1)

**Inclus** :
- Overlay flottant Tauri (toujours-visible, draggable, rétractable en pill)
- Détection et supervision de **Claude Code** uniquement
- Affichage de l'état : idle / thinking / awaiting-approval / awaiting-question
- Boutons graphiques : approve / deny / répondre à AskUserQuestion
- Navigation vers le terminal source (support minimum : GNOME Terminal, Konsole, Alacritty, Kitty)
- Alertes sonores configurables
- Packaging AppImage (CI GitHub Actions)

**Exclus du MVP** (phases ultérieures) :
- Autres agents (Codex, Cursor, Gemini CLI, Copilot, etc.)
- Rendu Markdown des plans
- Tray/AppIndicator mode
- Flatpak/.deb/.rpm
- Multi-monitor intelligent

## Scope v1.0

Tous les 10 agents supportés par l'original (Claude Code, Codex, Gemini CLI, Cursor, OpenCode, Droid, Qoder, Copilot, CodeBuddy, Kiro), packaging multi-distro, tray icon, rendu Markdown.

## Stack technique

- **Frontend** : Tauri v2 + React + TypeScript + Tailwind
- **Backend** : Rust (Tauri core)
- **IPC agents** : fichiers de contrôle (FIFO / Unix sockets / fichiers d'état dans `~/.claude/`, `~/.codex/`, etc.)
- **Packaging** : AppImage (MVP), Flatpak (v1.0), .deb/.rpm (v1.0)
- **CI** : GitHub Actions (build AppImage sur tag)

## Architecture haut-niveau

```
┌─────────────────────────────────────────┐
│  Tauri App (vibeisland-linux)           │
│  ┌───────────────────────────────────┐  │
│  │  Frontend (React + TS)            │  │
│  │  - Overlay panel                  │  │
│  │  - Agent cards                    │  │
│  │  - Approval UI                    │  │
│  └────────────┬──────────────────────┘  │
│               │ Tauri IPC                │
│  ┌────────────▼──────────────────────┐  │
│  │  Backend (Rust)                   │  │
│  │  - Agent adapters (trait-based)   │  │
│  │  - File watchers (notify crate)   │  │
│  │  - Terminal locator               │  │
│  │  - Sound player                   │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
         │
         │ watches / writes
         ▼
┌─────────────────────────────────────────┐
│  ~/.claude/, ~/.codex/, etc.            │
│  (hooks, state files, FIFOs)            │
└─────────────────────────────────────────┘
```

## Positionnement différenciateur

- **Anti-contexte switch** : même promesse que l'original, adaptée Linux
- **Cross-DE** : marche sur GNOME, KDE, Hyprland, Sway, i3
- **Open-source** (original est closed-source, payant)
- **Extensible** : adapter pattern pour ajouter de nouveaux agents facilement

## Monétisation

Open-source gratuit (MIT). Possibilité plus tard :
- Version Pro avec features avancées (multi-machine sync, analytics, équipes)
- Sponsoring GitHub / OpenCollective

## Synergies avec le portfolio

- **Figma AI** : Figma AI est un cockpit anti-chaos pour vibe coders → VibeIsland Linux est le complément terminal-side. Possibilité future : intégration bidirectionnelle.
- **Autopilot** : VibeIsland Linux peut afficher l'état des runs Autopilot (agents Claude Code dispatched sur issues GitHub).
- **Visibilité dev** : projet open-source → acquisition organique via GitHub/HackerNews/Reddit r/linux + r/programming.

## Phases de développement

- **Phase 0** — Setup repo, stack Tauri, CI basique (1-2 jours)
- **Phase 1** — MVP Claude Code uniquement, overlay flottant, approve/deny (1 semaine)
- **Phase 2** — Navigation terminal, AskUserQuestion, alertes sonores (1 semaine)
- **Phase 3** — Packaging AppImage auto, release v0.1 public (2-3 jours)
- **Phase 4** — Ajout Codex + Gemini CLI + Cursor (2 semaines)
- **Phase 5** — Autres agents + tray mode + Flatpak (3 semaines)
- **Phase 6** — v1.0 feature parity avec macOS original

## Risques

- **Détection agents non standardisée** : chaque CLI IA a son propre format de hooks/état. Nécessite reverse-engineering par agent.
- **Wayland vs X11** : l'overlay flottant toujours-visible est plus compliqué sur Wayland (pas de `always-on-top` universel). Fallback nécessaire.
- **Concurrence** : si l'auteur de VibeIsland macOS sort une version Linux officielle, on est mort. Mais il annonce "macOS only" → fenêtre ouverte.
- **Maintenance** : 10 agents × breaking changes fréquents = maintenance continue nécessaire.
