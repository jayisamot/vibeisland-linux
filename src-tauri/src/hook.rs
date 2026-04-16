//! CLI `hook <event>` subcommand.
//!
//! Claude Code (and future agents) invoke this binary as a hook — they
//! pipe a JSON payload on stdin and block on the exit code. We turn that
//! payload into an event file under `~/.vibeisland/events/`, then exit
//! quickly. The session store (issue #12) picks the file up via the
//! watcher (#11) and routes it to the right session.
//!
//! Contract:
//! - Reads stdin (may be empty / invalid JSON — never crashes)
//! - Writes one file per invocation, atomically (tmp + rename)
//! - Returns exit 0 in <100ms on the happy path
//! - Logs failures to `~/.vibeisland/hook.log` (append)

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;

use chrono::Utc;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use vibeisland_agents::HookPayload;

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    UserPromptSubmit,
    Stop,
    Notification,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreToolUse => "pre-tool-use",
            Self::PostToolUse => "post-tool-use",
            Self::UserPromptSubmit => "user-prompt-submit",
            Self::Stop => "stop",
            Self::Notification => "notification",
        }
    }
}

/// Entry point used by `main.rs` — returns the exit code.
pub fn run(event: HookEvent) -> i32 {
    match run_inner(event) {
        Ok(()) => 0,
        Err(err) => {
            let _ = log_error(event, &err);
            // Intentionally still exit 0: a broken hook must NEVER block
            // the underlying agent. Failures go to the log.
            0
        }
    }
}

fn run_inner(event: HookEvent) -> std::io::Result<()> {
    let home =
        home_dir().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no HOME"))?;
    let events_dir = home.join(".vibeisland").join("events");
    fs::create_dir_all(&events_dir)?;
    // 0700 (best effort; Windows doesn't care)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&events_dir, fs::Permissions::from_mode(0o700));
    }

    let mut stdin_buf = String::new();
    std::io::stdin().read_to_string(&mut stdin_buf).ok();

    let (payload, raw_payload) = if stdin_buf.trim().is_empty() {
        (None, None)
    } else {
        match serde_json::from_str::<serde_json::Value>(&stdin_buf) {
            Ok(v) => (Some(v), None),
            Err(_) => (None, Some(stdin_buf)),
        }
    };

    let ts = Utc::now();
    let id = uuid::Uuid::new_v4().to_string();
    let filename = format!("{}-{}.json", ts.format("%Y%m%dT%H%M%S%3f"), &id[..8]);

    let record = HookPayload {
        event: event.as_str().to_string(),
        id,
        timestamp: ts.to_rfc3339(),
        payload,
        raw_payload,
        session_id: env::var("CLAUDE_SESSION_ID")
            .ok()
            .or_else(|| env::var("VIBEISLAND_SESSION_ID").ok()),
        cwd: env::current_dir().ok().map(|p| p.display().to_string()),
        pid: process::id(),
        env: capture_env(),
    };

    write_atomic(&events_dir, &filename, &record)
}

fn capture_env() -> BTreeMap<String, String> {
    let keys = [
        "TERM_PROGRAM",
        "TERM",
        "KITTY_PID",
        "KITTY_WINDOW_ID",
        "KONSOLE_VERSION",
        "GNOME_TERMINAL_SCREEN",
        "ALACRITTY_WINDOW_ID",
        "COLORTERM",
        "WINDOWID",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "PPID",
    ];
    keys.iter()
        .filter_map(|k| env::var(k).ok().map(|v| ((*k).to_string(), v)))
        .collect()
}

fn write_atomic(
    dir: &std::path::Path,
    filename: &str,
    record: &HookPayload,
) -> std::io::Result<()> {
    let final_path = dir.join(filename);
    let tmp_path = dir.join(format!(".{filename}.tmp"));
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)?;
        let bytes = serde_json::to_vec(record).map_err(std::io::Error::other)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

fn log_error(event: HookEvent, err: &std::io::Error) -> std::io::Result<()> {
    let home = match home_dir() {
        Some(h) => h,
        None => return Ok(()),
    };
    let dir = home.join(".vibeisland");
    fs::create_dir_all(&dir)?;
    let log_path = dir.join("hook.log");
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(
        f,
        "{} event={} pid={} err={}",
        Utc::now().to_rfc3339(),
        event.as_str(),
        process::id(),
        err,
    )
}

fn home_dir() -> Option<PathBuf> {
    // $VIBEISLAND_HOME overrides for tests and portable installs.
    if let Ok(h) = env::var("VIBEISLAND_HOME") {
        return Some(PathBuf::from(h));
    }
    directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kebab_matches_as_str() {
        for e in [
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::UserPromptSubmit,
            HookEvent::Stop,
            HookEvent::Notification,
        ] {
            let serialized = serde_json::to_string(&e).unwrap();
            let expected = format!("\"{}\"", e.as_str());
            assert_eq!(serialized, expected);
        }
    }

    #[test]
    fn write_atomic_creates_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let record = HookPayload {
            event: "pre-tool-use".into(),
            id: "abc-123".into(),
            timestamp: "2026-04-16T00:00:00Z".into(),
            payload: Some(serde_json::json!({"tool_name": "Bash"})),
            raw_payload: None,
            session_id: None,
            cwd: Some("/tmp".into()),
            pid: 42,
            env: Default::default(),
        };
        write_atomic(dir.path(), "x.json", &record).unwrap();
        let content = fs::read_to_string(dir.path().join("x.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["event"], "pre-tool-use");
        assert_eq!(v["payload"]["tool_name"], "Bash");
    }

    #[test]
    fn run_with_empty_stdin_writes_record_with_null_payload() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("VIBEISLAND_HOME", tmp.path());
        let result = run_inner(HookEvent::Stop);
        assert!(result.is_ok());
        let events = fs::read_dir(tmp.path().join(".vibeisland/events")).unwrap();
        let files: Vec<_> = events.filter_map(|e| e.ok()).collect();
        assert_eq!(files.len(), 1);
        let content = fs::read_to_string(files[0].path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["event"], "stop");
        assert!(v["payload"].is_null());
        std::env::remove_var("VIBEISLAND_HOME");
    }
}
