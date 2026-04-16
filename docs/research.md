# VibeIsland Linux — Research

## Concurrents directs

### VibeIsland (macOS) — https://vibeisland.app/
- Closed-source, payant
- macOS 14+ Apple Silicon only
- Dynamic Island native (encoche MacBook) + barre flottante écran externe
- Supporte 10 agents : Claude Code, Codex, Gemini CLI, Cursor, OpenCode, Droid, Qoder, Copilot, CodeBuddy, Kiro
- < 50 MB RAM, Swift natif
- Auto-config zéro-setup
- Features : approve/deny GUI, AskUserQuestion, nav terminal (13+ émulateurs), rendu Markdown plans, alertes sonores 8-bit

**Verdict** : pas de concurrent direct sur Linux. L'auteur semble commit macOS-only.

## Concurrents indirects

### Claude Code native hooks
- Claude Code a un système de hooks (`PreToolUse`, `PostToolUse`, etc.)
- On peut intercepter via hooks bash et écrire dans un fichier que VibeIsland lit
- Pas d'UI, purement script → notre opportunité = UI par-dessus

### Warp Terminal (macOS/Linux)
- Terminal avec assistant IA intégré
- Mais ne supervise pas d'autres agents → différent scope

### Zed Editor
- Éditeur avec agents IA
- In-IDE, pas supervision multi-agents terminal-side

### Tmux + scripts custom
- Les power users bricolent avec tmux + notifications libnotify
- Notre cible = rendre ça accessible non-bricoleurs

## Patterns overlay Linux

### Options pour "always-on-top floating panel"

**X11** :
- `_NET_WM_STATE_ABOVE` (marche partout)
- Tauri supporte via `alwaysOnTop: true`

**Wayland** :
- Pas de `always-on-top` standard dans le protocole
- `wlr-layer-shell` (wlroots : Sway, Hyprland) → OK
- GNOME Wayland : ne supporte pas layer-shell → dégradé (fenêtre normale)
- KDE Plasma 6 : support partiel

**Solution pragmatique MVP** :
- Tauri `alwaysOnTop: true` sur X11 et compositeurs wlroots
- Fallback tray icon pour GNOME Wayland (deuxième phase)

### Position et draggability

- Tauri supporte `decorations: false` + drag region CSS
- Mémoriser position dans config locale (`~/.config/vibeisland-linux/position.json`)

### Tray / AppIndicator

- Tauri v2 a un support tray natif (multi-plateforme)
- Fallback pour users qui veulent discret
- Nécessite AppIndicator sur Ubuntu/GNOME (extension gnome-shell-extension-appindicator)

## Détection d'agents — approche par agent

### Claude Code
- **Config** : `~/.claude/settings.json` (global), `.claude/settings.json` (projet)
- **Hooks** : `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`, etc.
- **Approche** : injecter nos hooks bash qui écrivent l'état dans `~/.vibeisland/claude/<session-id>.json`
- **AskUserQuestion** : détectable via hook `PreToolUse` sur outil `AskUserQuestion`
- **Session tracking** : Claude Code a un concept de session, on peut le récupérer via env vars ou transcript

### Codex (OpenAI CLI)
- Moins documenté publiquement
- À investiguer : logs, transcript files, workdir détection

### Gemini CLI (Google)
- CLI officielle Google
- Config dans `~/.gemini/`
- Hooks ? À vérifier

### Cursor
- App Electron, pas CLI pure
- Expose peut-être un API/IPC local
- Possiblement hors scope MVP (pas terminal-based)

### Copilot CLI
- `gh copilot` — CLI GitHub
- Pas de hooks documentés
- À investiguer

### Autres (OpenCode, Droid, Qoder, CodeBuddy, Kiro)
- Recherche approfondie à faire phase 4+

## Navigation vers terminal source

Chaque émulateur a son mécanisme :
- **GNOME Terminal** : D-Bus `org.gnome.Terminal`
- **Konsole** : D-Bus `org.kde.konsole`
- **Alacritty** : pas de D-Bus, fallback `wmctrl` / `xdotool` via window title
- **Kitty** : socket IPC `kitty @`
- **Wezterm** : `wezterm cli`
- **tmux** : `tmux switch-client -t <session>:<window>`
- **Ghostty** : peu mature sur Linux

**Approche** : adapter pattern Rust, un module par émulateur, fallback générique `wmctrl` sur titre de fenêtre.

## Alertes sonores

- `playback-rs` ou `rodio` crate Rust
- Samples 8-bit embeddés dans le binaire
- Config volume dans settings

## Packaging Linux

### AppImage (MVP)
- Portable, marche partout
- `cargo-tauri` supporte nativement
- Distribution via GitHub Releases

### Flatpak (v1.0)
- Store Linux moderne (Flathub)
- Sandboxing → attention aux accès fichiers (`~/.claude/`, etc.)
- Nécessite manifest Flatpak

### .deb / .rpm (v1.0)
- Pour users qui veulent install via package manager
- `cargo-tauri` supporte

## Pistes community / acquisition

- **r/linux, r/programming, r/commandline** sur Reddit
- **HackerNews** (Show HN)
- **Lobste.rs** (techies)
- **GitHub trending** (si bonne étoile initiale)
- **Twitter dev** (cibler followers de l'auteur VibeIsland macOS)
- **Blog post** comparaison VibeIsland macOS vs Linux
- **Démo vidéo** courte (30s) overlay en action

## Questions ouvertes

1. **Wayland GNOME** : accepter le dégradé tray-only ou trouver hack (portails XDG) ?
2. **Auto-config** : peut-on vraiment zéro-config sur Linux vu la fragmentation ?
3. **Permissions** : Flatpak sandboxing bloque-t-il l'accès aux hooks d'agents ?
4. **Session multi-projets** : comment gérer 5 Claude Code simultanés dans différents repos ?
