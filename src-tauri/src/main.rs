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
    /// Ingest a hook event from an AI coding agent and write it to
    /// ~/.vibeisland/events/ for the overlay to pick up.
    Hook {
        /// Event name (pre-tool-use, post-tool-use, user-prompt-submit,
        /// stop, notification).
        #[arg(value_enum)]
        event: HookEvent,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Hook { event }) => std::process::exit(hook::run(event)),
        None => vibeisland_linux_lib::run(),
    }
}
