# Contributing to VibeIsland Linux

Thanks for looking! This project is early-stage and moving fast — the goal is a usable v0.1 AppImage that supervises Claude Code. All help welcome, especially:

- Linux compat fixes (Wayland, HiDPI, Hyprland, Sway, i3)
- Terminal emulator locators
- Agent adapters beyond Claude Code (Codex, Gemini CLI, ...)
- Sound design and icons

## Before you start

1. Read [docs/brief.md](docs/brief.md) and [docs/architecture.md](docs/architecture.md).
2. Check [docs/roadmap.md](docs/roadmap.md) and the [issues list](https://github.com/jayisamot/vibeisland-linux/issues). Pick something labeled `auto-execute` if you want a self-contained task; `needs-human` means there's a design or UX call to make first.
3. Comment on the issue so we know you're on it (avoids duplicate work).

## Workflow

This repo uses a **trunk-based** flow with version branches:

- `master` = last released version
- `dev/vN` = active development branch for the next release (e.g. `dev/v0.1`)
- No branch per issue. Commit directly to the current `dev/vN`.
- A single PR `dev/vN → master` when the version is ready. It's never blocking during the rodage phase.

If you're external, fork and send a PR against `dev/v0.1`.

## Commit style

Conventional Commits with the issue number:

```
feat(#12): session store atomic write
fix(#9): handle missing ~/.claude/settings.json gracefully
chore(#4): bump tauri-cli to 2.1.0
```

One commit per issue when possible — this keeps the changelog clean.

## Code quality

- Rust: `cargo fmt` + `cargo clippy --all-targets -- -D warnings`
- TypeScript: `npm run lint` + `tsc --noEmit`
- Pre-commit hooks run these automatically (see [#7](https://github.com/jayisamot/vibeisland-linux/issues/7)).

## Tests

- Unit tests: `cargo test` and `npm test`
- Integration / E2E: see [#20](https://github.com/jayisamot/vibeisland-linux/issues/20)

Aim to add at least one test per feature — not necessarily TDD, but enough to keep the CI honest.

## Release

Tagging `vX.Y.Z` on `master` triggers the release workflow ([#35](https://github.com/jayisamot/vibeisland-linux/issues/35)). Don't tag yourself; flag the maintainer.

## Code of conduct

Be kind. Review code, not people. If something feels off, open an issue or DM the maintainer.

## License

By contributing, you agree your work ships under the [MIT License](LICENSE).
