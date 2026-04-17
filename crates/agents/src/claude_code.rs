//! Claude Code adapter.
//!
//! Installs / removes hook entries in `~/.claude/settings.json`. Design
//! choices (documented in the issue #9 discussion):
//!
//! - **Merge on conflict, never override.** If the user already has hooks
//!   for PreToolUse / UserPromptSubmit / Stop, we APPEND our entry to
//!   the same bucket rather than replace. Our entry carries a well-known
//!   command prefix (`VIBEISLAND_MARKER`) so uninstall can identify it.
//! - **Backup once.** Before the first mutation we copy the original to
//!   `settings.json.vibeisland-backup`. We never overwrite an existing
//!   backup — if one is already there, we assume it's the real one.
//! - **Binary path = current exe.** We resolve the VibeIsland binary
//!   via `std::env::current_exe()` at install time and bake the absolute
//!   path into the hook command, so hooks keep working when the app
//!   isn't on the user's PATH.
//!
//! Approve/deny/answer are no-ops in this adapter: the response file
//! written by the IPC layer (issue #14) is what Claude Code actually
//! reads from the blocked hook process.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::fs;

use crate::{Agent, AgentError, AgentResult, EventKind};

/// Well-known identifier injected in every hook command we install.
/// `ClaudeCodeAgent::uninstall` uses it to recognize our entries.
pub const VIBEISLAND_MARKER: &str = "# VIBEISLAND_HOOK";

/// Claude Code hook event names — these are the JSON keys under
/// `settings.hooks.*`. Kept in sync with
/// <https://docs.claude.com/claude-code/hooks>.
const EVENT_KEYS: &[(&str, &str)] = &[
    ("PreToolUse", "pre-tool-use"),
    ("PostToolUse", "post-tool-use"),
    ("UserPromptSubmit", "user-prompt-submit"),
    ("Stop", "stop"),
    ("Notification", "notification"),
];

pub struct ClaudeCodeAgent {
    settings_path: PathBuf,
    binary_path: PathBuf,
}

impl ClaudeCodeAgent {
    /// Construct the adapter with auto-discovery of the settings file
    /// and the current-exe path.
    pub fn new() -> std::io::Result<Self> {
        let settings_path = default_settings_path()?;
        let binary_path = std::env::current_exe()?;
        Ok(Self {
            settings_path,
            binary_path,
        })
    }

    /// Test-only constructor letting the caller pin both paths.
    #[doc(hidden)]
    pub fn with_paths(settings_path: PathBuf, binary_path: PathBuf) -> Self {
        Self {
            settings_path,
            binary_path,
        }
    }

    fn backup_path(&self) -> PathBuf {
        let mut p = self.settings_path.clone();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("settings.json")
            .to_string();
        p.set_file_name(format!("{name}.vibeisland-backup"));
        p
    }

    fn hook_command(&self, hook_event: &str) -> String {
        format!(
            "{bin} hook {event} {marker}",
            bin = shell_escape(&self.binary_path),
            event = hook_event,
            marker = VIBEISLAND_MARKER,
        )
    }

    fn is_our_entry(&self, hook_entry: &Value) -> bool {
        hook_entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|list| {
                list.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(|c| c.contains(VIBEISLAND_MARKER))
                })
            })
            .unwrap_or(false)
    }
}

fn default_settings_path() -> std::io::Result<PathBuf> {
    if let Ok(override_path) = std::env::var("VIBEISLAND_CLAUDE_SETTINGS") {
        return Ok(PathBuf::from(override_path));
    }
    let base = directories::BaseDirs::new()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no HOME"))?;
    Ok(base.home_dir().join(".claude").join("settings.json"))
}

/// Shell-escape a path for embedding inside a hook `command` string.
/// Claude Code passes the command to `/bin/sh -c`, so spaces and
/// metacharacters in the path would break it otherwise.
fn shell_escape(p: &Path) -> String {
    let s = p.to_string_lossy();
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'))
    {
        s.into_owned()
    } else {
        // POSIX single-quote escaping: wrap in '...' and close-reopen
        // for any embedded single quote.
        let escaped = s.replace('\'', "'\\''");
        format!("'{escaped}'")
    }
}

#[async_trait]
impl Agent for ClaudeCodeAgent {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn supported_events(&self) -> Vec<EventKind> {
        vec![
            EventKind::PreToolUse,
            EventKind::PostToolUse,
            EventKind::UserPromptSubmit,
            EventKind::Stop,
            EventKind::Notification,
        ]
    }

    async fn install(&self) -> AgentResult<()> {
        // Ensure parent directory exists (a fresh install might have
        // never launched Claude Code).
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Read existing settings or start with an empty object.
        let (mut root, had_file) = match fs::read(&self.settings_path).await {
            Ok(bytes) => {
                let v = serde_json::from_slice::<Value>(&bytes).unwrap_or_else(|_| json!({}));
                (v, true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (json!({}), false),
            Err(e) => return Err(AgentError::Io(e)),
        };

        // Backup once (only if the file existed AND we haven't backed up yet).
        if had_file && !fs::try_exists(self.backup_path()).await.unwrap_or(false) {
            fs::copy(&self.settings_path, self.backup_path()).await?;
        }

        if !root.is_object() {
            root = json!({});
        }
        let hooks = root
            .as_object_mut()
            .unwrap()
            .entry("hooks")
            .or_insert_with(|| json!({}));

        for (key, event_kebab) in EVENT_KEYS {
            let bucket = hooks
                .as_object_mut()
                .ok_or_else(|| AgentError::Other("hooks must be an object".into()))?
                .entry((*key).to_string())
                .or_insert_with(|| json!([]));
            let array = bucket
                .as_array_mut()
                .ok_or_else(|| AgentError::Other(format!("hooks.{key} must be an array")))?;

            // Remove stale VibeIsland entries so re-install updates the path.
            array.retain(|entry| !self.is_our_entry(entry));
            array.push(json!({
                "matcher": "*",
                "hooks": [
                    { "type": "command", "command": self.hook_command(event_kebab) }
                ]
            }));
        }

        let rendered = serde_json::to_vec_pretty(&root)?;
        write_atomic(&self.settings_path, &rendered).await?;
        Ok(())
    }

    async fn uninstall(&self) -> AgentResult<()> {
        let bytes = match fs::read(&self.settings_path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(AgentError::Io(e)),
        };
        let mut root: Value = serde_json::from_slice(&bytes)?;
        let Some(hooks) = root
            .as_object_mut()
            .and_then(|m| m.get_mut("hooks"))
            .and_then(|h| h.as_object_mut())
        else {
            return Ok(());
        };
        for (key, _) in EVENT_KEYS {
            if let Some(Value::Array(arr)) = hooks.get_mut(*key) {
                arr.retain(|entry| !self.is_our_entry(entry));
                if arr.is_empty() {
                    hooks.remove(*key);
                }
            }
        }
        // If `hooks` is now empty, drop the whole key to keep the file
        // as clean as we found it.
        if hooks.is_empty() {
            root.as_object_mut().unwrap().remove("hooks");
        }
        let rendered = serde_json::to_vec_pretty(&root)?;
        write_atomic(&self.settings_path, &rendered).await?;
        Ok(())
    }

    async fn is_installed(&self) -> bool {
        let Ok(bytes) = fs::read(&self.settings_path).await else {
            return false;
        };
        let Ok(root) = serde_json::from_slice::<Value>(&bytes) else {
            return false;
        };
        let Some(hooks) = root.get("hooks").and_then(|h| h.as_object()) else {
            return false;
        };
        hooks.values().any(|bucket| {
            bucket
                .as_array()
                .map(|arr| arr.iter().any(|e| self.is_our_entry(e)))
                .unwrap_or(false)
        })
    }

    async fn approve(&self, _session_id: &str, _action_id: &str) -> AgentResult<()> {
        // The response file written by IPC (issue #14) is what unblocks
        // the hook. Nothing agent-specific to do here.
        Ok(())
    }

    async fn deny(&self, _session_id: &str, _action_id: &str) -> AgentResult<()> {
        Ok(())
    }

    async fn answer(
        &self,
        _session_id: &str,
        _question_id: &str,
        _answer: &str,
    ) -> AgentResult<()> {
        Ok(())
    }
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).await?;
    fs::rename(tmp, path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_adapter(dir: &tempfile::TempDir) -> ClaudeCodeAgent {
        ClaudeCodeAgent::with_paths(
            dir.path().join("settings.json"),
            PathBuf::from("/usr/local/bin/vibeisland-linux"),
        )
    }

    #[tokio::test]
    async fn install_creates_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let agent = temp_adapter(&dir);
        assert!(!agent.is_installed().await);
        agent.install().await.unwrap();
        assert!(agent.is_installed().await);

        let settings: Value =
            serde_json::from_slice(&fs::read(dir.path().join("settings.json")).await.unwrap())
                .unwrap();
        let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        let cmd = pre[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("hook pre-tool-use"));
        assert!(cmd.contains(VIBEISLAND_MARKER));
    }

    #[tokio::test]
    async fn install_preserves_existing_non_vibeisland_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let existing = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{ "type": "command", "command": "/usr/local/bin/my-audit-log" }]
                }]
            },
            "model": "claude-opus-4-6"
        });
        fs::write(&path, serde_json::to_vec_pretty(&existing).unwrap())
            .await
            .unwrap();

        let agent = temp_adapter(&dir);
        agent.install().await.unwrap();

        let settings: Value = serde_json::from_slice(&fs::read(&path).await.unwrap()).unwrap();
        let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "user hook + vibeisland hook");
        assert_eq!(settings["model"], "claude-opus-4-6");
        // Backup created and matches the original exactly.
        let backup: Value = serde_json::from_slice(
            &fs::read(dir.path().join("settings.json.vibeisland-backup"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(backup, existing);
    }

    #[tokio::test]
    async fn uninstall_removes_only_our_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "hooks": {
                    "PreToolUse": [{
                        "matcher": "Bash",
                        "hooks": [{ "type": "command", "command": "/usr/local/bin/my-audit-log" }]
                    }]
                }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

        let agent = temp_adapter(&dir);
        agent.install().await.unwrap();
        agent.uninstall().await.unwrap();

        let settings: Value = serde_json::from_slice(&fs::read(&path).await.unwrap()).unwrap();
        let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        let cmd = pre[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(cmd, "/usr/local/bin/my-audit-log");
    }

    #[tokio::test]
    async fn uninstall_is_idempotent_and_handles_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let agent = temp_adapter(&dir);
        // Missing file — should not error.
        agent.uninstall().await.unwrap();
        // After install then uninstall, another uninstall is still fine.
        agent.install().await.unwrap();
        agent.uninstall().await.unwrap();
        agent.uninstall().await.unwrap();
        assert!(!agent.is_installed().await);
    }

    #[tokio::test]
    async fn reinstall_replaces_stale_entries_instead_of_duplicating() {
        let dir = tempfile::tempdir().unwrap();
        let agent = temp_adapter(&dir);
        agent.install().await.unwrap();
        agent.install().await.unwrap();
        let settings: Value =
            serde_json::from_slice(&fs::read(dir.path().join("settings.json")).await.unwrap())
                .unwrap();
        let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1, "no duplicates on re-install");
    }

    #[tokio::test]
    async fn backup_is_not_overwritten_on_reinstall() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{\"model\":\"first\"}").await.unwrap();

        let agent = temp_adapter(&dir);
        agent.install().await.unwrap();
        // Simulate user editing settings AFTER install.
        fs::write(&path, b"{\"model\":\"second\"}").await.unwrap();
        agent.install().await.unwrap();

        let backup = fs::read_to_string(dir.path().join("settings.json.vibeisland-backup"))
            .await
            .unwrap();
        assert!(
            backup.contains("first"),
            "backup should still hold the original first-install snapshot"
        );
    }

    #[test]
    fn shell_escape_wraps_paths_with_spaces() {
        let p = PathBuf::from("/opt/with space/bin");
        assert_eq!(shell_escape(&p), "'/opt/with space/bin'");
        let simple = PathBuf::from("/usr/local/bin/vibeisland-linux");
        assert_eq!(shell_escape(&simple), "/usr/local/bin/vibeisland-linux");
    }
}
