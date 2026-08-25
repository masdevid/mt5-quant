# Contributing to MT5-Quant

Thanks for your interest in improving MT5-Quant! This document explains how to
contribute and what we expect from contributions.

## How to contribute

1. **Fork** the repository and create a topic branch from `master`.
2. Make your change, keeping it focused (one logical change per PR).
3. Open a **pull request against `master`**. A pull request template
   (`.github/PULL_REQUEST_TEMPLATE.md`) is provided — please fill it in.
4. Ensure CI passes (build, `cargo test`, clippy, formatting, cargo-deny,
   CodeQL). Maintainers review and merge once checks and review are satisfied.

## Development setup

- Install Rust (stable toolchain) and, for full backtests, Wine/MT5 as described
  in `README.md` and `docs/QUICKSTART.md`.
- Build the release binary with either:
  - `bash scripts/build-rust.sh`, or
  - `cargo build --release`
- The binary is produced at `target/release/mt5-quant`.

## Testing requirement

- **Non-trivial changes must include tests.** New functionality should come with
  unit or integration tests where practical.
- `cargo test` runs automatically in CI (`rust.yml`) on every push and pull
  request, so a change that breaks tests cannot merge.

## Commit style

- Follow **Conventional Commits** (`feat:`, `fix:`, `docs:`, `refactor:`,
  `test:`, `release:`), as documented in `AGENTS.md`.

## Issue tracker

- Bugs and enhancement requests are tracked on the project's
  [GitHub Issues](https://github.com/masdevid/mt5-quant/issues), with
  bug-report and feature-request templates under `.github/ISSUE_TEMPLATE/`.
