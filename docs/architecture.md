# VibeIsland Linux — Architecture

## Structure du projet

```
vibeisland-linux/
├── docs/                           # brief, research, architecture, etc.
├── src-tauri/                      # Rust backend
│   ├── src/
│   │   ├── main.rs                 # entry point
│   │   ├── agents/                 # agent adapters
│   │   │   ├── mod.rs              # trait Agent
│   │   │   ├── claude_code.rs      # Claude Code adapter
│   │   │   ├── codex.rs            # (phase 4)
│   │   │   └── ...
│   │   ├── watcher.rs              # file watchers (notify crate)
│   │   ├── terminal/               # terminal locators
│   │   │   ├── mod.rs              # trait TerminalLocator
│   │   │   ├── gnome_terminal.rs
│   │   │   ├── konsole.rs
│   │   │   ├── kitty.rs
│   │   │   ├── alacritty.rs
│   │   │   └── ...
│   │   ├── sound.rs                # sound player (rodio)
│   │   ├── config.rs               # user config (JSON in ~/.config/vibeisland-linux/)
│   │   └── ipc.rs                  # Tauri commands exposed to frontend
│   ├── tauri.conf.json             # Tauri config
│   ├── Cargo.toml
│   └── build.rs
├── src/                            # React frontend
│   ├── App.tsx
│   ├── main.tsx
│   ├── components/
│   │   ├── OverlayPanel.tsx        # main floating panel
│   │   ├── AgentCard.tsx           # one card per agent session
│   │   ├── ApprovalPrompt.tsx      # approve/deny UI
│   │   ├── AskQuestion.tsx         # AskUserQuestion UI
│   │   └── PillCollapsed.tsx       # collapsed state
│   ├── hooks/
│   │   ├── useAgentState.ts        # subscribe to agent events
│   │   └── useConfig.ts
│   ├── types/
│   │   └── agent.ts                # shared types with Rust
│   └── styles/
│       └── globals.css             # Tailwind
├── public/
│   ├── sounds/                     # 8-bit alert samples
│   └── icons/
├── .github/
│   └── workflows/
│       ├── ci.yml                  # build + test on PR
│       └── release.yml             # build AppImage on tag
├── package.json
├── tsconfig.json
├── tailwind.config.js
├── vite.config.ts
└── README.md
```

## Trait `Agent` (Rust)

```rust
pub trait Agent: Send + Sync {
    /// Unique identifier (e.g. "claude-code", "codex")
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Install hooks/config needed to start monitoring
    fn install(&self) -> Result<()>;

    /// Uninstall hooks/config
    fn uninstall(&self) -> Result<()>;

    /// Return current state of all active sessions
    fn sessions(&self) -> Result<Vec<AgentSession>>;

    /// Approve a pending action
    fn approve(&self, session_id: &str, action_id: &str) -> Result<()>;

    /// Deny a pending action
    fn deny(&self, session_id: &str, action_id: &str) -> Result<()>;

    /// Answer an AskUserQuestion
    fn answer(&self, session_id: &str, question_id: &str, answer: String) -> Result<()>;

    /// Get the directory / FIFOs to watch
    fn watch_paths(&self) -> Vec<PathBuf>;
}

pub struct AgentSession {
    pub id: String,
    pub agent_id: String,
    pub state: SessionState,
    pub cwd: PathBuf,
    pub terminal: Option<TerminalInfo>,
    pub pending_action: Option<PendingAction>,
    pub last_activity: DateTime<Utc>,
}

pub enum SessionState {
    Idle,
    Thinking,
    AwaitingApproval,
    AwaitingQuestion,
    Error,
}

pub enum PendingAction {
    ToolPermission { tool: String, args: serde_json::Value },
    Question { prompt: String, options: Vec<String> },
}
```

## Claude Code adapter — détails

### Install hooks

Au premier lancement, on modifie `~/.claude/settings.json` pour ajouter :

```json
{
  "hooks": {
    "PreToolUse": "vibeisland-linux hook pre-tool-use",
    "UserPromptSubmit": "vibeisland-linux hook user-prompt-submit",
    "Stop": "vibeisland-linux hook stop"
  }
}
```

Le binaire `vibeisland-linux` expose un sous-commande `hook` qui écrit dans `~/.vibeisland/events/<timestamp>-<uuid>.json`.

### État des sessions

Chaque session Claude Code = une ligne dans `~/.vibeisland/sessions.json` :

```json
{
  "sessions": {
    "abc-123": {
      "agent_id": "claude-code",
      "cwd": "/home/jay/babytracker",
      "terminal": {
        "emulator": "kitty",
        "window_id": "0x1234567",
        "tab_id": 2
      },
      "state": "awaiting_approval",
      "pending_action": {
        "type": "tool_permission",
        "tool": "Bash",
        "args": { "command": "rm -rf /" }
      },
      "last_activity": "2026-04-16T12:34:56Z"
    }
  }
}
```

### Watcher

Le backend Rust utilise `notify` pour surveiller `~/.vibeisland/events/` et `~/.vibeisland/sessions.json`. Chaque changement → event Tauri émis vers le frontend.

### Approve/Deny mécanisme

Problème : Claude Code attend la réponse d'un hook. Notre hook doit bloquer jusqu'à ce que l'user clique approve/deny dans l'overlay.

Solution :
1. Hook bloque en attendant un fichier `~/.vibeisland/responses/<action-id>` (via `inotifywait` ou polling).
2. UI overlay écrit ce fichier avec `{"decision": "approve" | "deny"}`.
3. Hook lit et renvoie le code de sortie approprié à Claude Code (0 = approve, non-zero = deny).

## IPC Frontend ↔ Backend (Tauri commands)

```rust
#[tauri::command]
async fn list_sessions() -> Result<Vec<AgentSession>, String>;

#[tauri::command]
async fn approve(session_id: String, action_id: String) -> Result<(), String>;

#[tauri::command]
async fn deny(session_id: String, action_id: String) -> Result<(), String>;

#[tauri::command]
async fn answer_question(session_id: String, question_id: String, answer: String) -> Result<(), String>;

#[tauri::command]
async fn focus_terminal(session_id: String) -> Result<(), String>;

#[tauri::command]
async fn get_config() -> Result<Config, String>;

#[tauri::command]
async fn set_config(config: Config) -> Result<(), String>;
```

Events émis par backend :

- `session:updated` → `AgentSession`
- `session:new` → `AgentSession`
- `session:closed` → `session_id: String`
- `sound:play` → `sound_name: String`

## Terminal locator — trait

```rust
pub trait TerminalLocator {
    fn emulator(&self) -> &'static str;

    /// Find terminal window/tab hosting the given PID
    fn locate(&self, pid: u32) -> Option<TerminalLocation>;

    /// Focus the terminal window/tab
    fn focus(&self, location: &TerminalLocation) -> Result<()>;
}

pub struct TerminalLocation {
    pub emulator: String,
    pub window_id: Option<String>,
    pub tab_id: Option<u32>,
    pub tmux_session: Option<String>,
    pub tmux_window: Option<String>,
}
```

Fallback générique : `wmctrl -l` + match sur le titre de la fenêtre + PID.

## Config user

Fichier : `~/.config/vibeisland-linux/config.json`

```json
{
  "overlay": {
    "position": { "x": 100, "y": 50 },
    "always_on_top": true,
    "start_collapsed": false,
    "auto_hide_idle_seconds": 60
  },
  "sounds": {
    "enabled": true,
    "volume": 0.8,
    "theme": "8bit"
  },
  "agents": {
    "claude_code": { "enabled": true },
    "codex": { "enabled": false }
  },
  "terminal_focus": {
    "enabled": true,
    "prefer_emulator": "kitty"
  }
}
```

## Sécurité

- **Hooks sandbox** : les hooks que VibeIsland installe ne doivent pas pouvoir exfiltrer les prompts/réponses ailleurs que sur le FS local de l'user.
- **Pas de network call** dans le MVP (tout local).
- **Audit open-source** : code source entièrement auditable, pas d'obfuscation.
- **Permissions Tauri** : restreindre `allowlist` Tauri au strict nécessaire.

## Performance

- **Cible RAM** : < 80 MB (moins strict que les 50 MB de l'original Swift, normal car Tauri + Webview)
- **Cible CPU idle** : < 0.1%
- **Cible startup** : < 500 ms
- **Watchers** : utiliser `notify` en mode event-based, jamais de polling

## Tests

- **Unit tests Rust** : `cargo test` pour adapters agents + terminal locators
- **Integration tests** : scripts shell qui simulent un hook Claude Code, vérifient l'état
- **E2E** : Playwright sur la webview Tauri (phase 2)
