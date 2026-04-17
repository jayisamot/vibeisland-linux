// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::{Parser, Subcommand};
use vibeisland_linux_lib::hook::{self, HookEvent};

#[derive(Parser)]
#[command(
    name = "vibeisland-linux",
    about = "Supervise AI coding agents from a floating overlay",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Ingest a hook event from an AI coding agent.
    Hook {
        #[arg(value_enum)]
        event: HookEvent,
        /// Trailing arguments passed by Claude Code (matcher etc.) — ignored.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        _rest: Vec<String>,
    },
    /// Install hooks for the named agent into its user-global config.
    Install { agent: String },
    /// Remove hooks for the named agent.
    Uninstall { agent: String },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Hook { event, .. }) => std::process::exit(hook::run(event)),
        Some(Command::Install { agent }) => std::process::exit(run_install_cli(&agent)),
        Some(Command::Uninstall { agent }) => std::process::exit(run_uninstall_cli(&agent)),
        None => vibeisland_linux_lib::run(),
    }
}

fn run_install_cli(agent: &str) -> i32 {
    let Some(adapter) = resolve_agent(agent) else {
        eprintln!("unknown agent: {agent}");
        return 1;
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    match rt.block_on(adapter.install()) {
        Ok(()) => {
            println!("installed hooks for {}", adapter.name());
            0
        }
        Err(e) => {
            eprintln!("install failed: {e}");
            2
        }
    }
}

fn run_uninstall_cli(agent: &str) -> i32 {
    let Some(adapter) = resolve_agent(agent) else {
        eprintln!("unknown agent: {agent}");
        return 1;
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    match rt.block_on(adapter.uninstall()) {
        Ok(()) => {
            println!("uninstalled hooks for {}", adapter.name());
            0
        }
        Err(e) => {
            eprintln!("uninstall failed: {e}");
            2
        }
    }
}

fn resolve_agent(id: &str) -> Option<Box<dyn vibeisland_agents::Agent>> {
    match id {
        "claude-code" => vibeisland_agents::ClaudeCodeAgent::new()
            .ok()
            .map(|a| Box::new(a) as Box<dyn vibeisland_agents::Agent>),
        _ => None,
    }
}
