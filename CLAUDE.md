# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# claude-code-statusline

A Claude Code status line as a native Rust binary. It reads the session JSON on stdin and prints two ANSI-colored lines. It targets Windows x64, Linux x64 (static musl, also used in WSL) and macOS (arm64 and x64).

## Commands

```sh
cargo build --release
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
echo '{"model":{"display_name":"Opus"}}' | cargo run -q   # try it by hand
```

## Layout

- `src/main.rs`: entry point. Reads stdin, calls git and render, writes stdout with no trailing newline. `--version` prints `STATUSLINE_VERSION` from build time (`dev` locally).
- `src/render.rs`: pure formatting (`parse_input`, `cwd`, `shorten_home`, `render`). Fields are read with `Value::pointer`; missing, null and non-numeric values are skipped, and non-object input renders as empty.
- `src/git.rs`: one `git status --porcelain=v2 --branch --untracked-files=no` call with a 2 s timeout, and the pure `parse_status`.
- `docs/sample.svg`: the README's colored preview. Update it if the output format changes.

## Conventions & Constraints

- The output format is pinned by the golden test `full_input_matches_exact_output` in `render.rs`. Change the format deliberately and update the test, README and `docs/sample.svg` together.
- Startup speed is the point of the project. Avoid adding dependencies or work that runs on every invocation.
- Code must build and behave the same on all three OSes. Platform differences go behind `cfg!(windows)` / `#[cfg(windows)]` (home folder variable, case-insensitive paths, `CREATE_NO_WINDOW`).
- **Versioning is CalVer `YYYY.MM.DD`** (UTC, with a `.N` suffix for repeated same-day releases). `Cargo.toml` stays at `0.0.0`; the release workflow injects the version through `STATUSLINE_VERSION`.
- **Branch flow**: a single `master` branch. CI (`.github/workflows/ci.yml`) runs on pushes and PRs. Releases are manual: run the **Release** workflow from the Actions tab, which computes the version, builds all four targets, tags `vYYYY.MM.DD` and publishes the GitHub release with auto-generated notes.
- `CHANGELOG.md` follows Keep a Changelog; add user-visible changes under `[Unreleased]`.
- **Attribution**: never add a `Co-Authored-By: Claude` trailer to commits. PR descriptions and PR comments written by Claude may carry a marker (e.g. a "🤖 Generated with Claude Code" footer) so they can be told apart from ones written by a human.
