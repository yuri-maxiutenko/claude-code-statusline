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
- `src/render.rs`: pure formatting (`parse_input`, `cwd`, `shorten_home`, `money`, `render`). Fields are read with `Value::pointer`; missing, null and non-numeric values are skipped, and non-object input renders as empty. `money` rounds the cost on its shortest decimal form (what Claude Code sent), half away from zero, never on the f64 itself. Don't replace it with `{:.2}`, which rounds `0.145` down to `0.14`.
- `src/git.rs`: one `git status --porcelain=v2 --branch --untracked-files=no` call with a 2 s timeout, and the pure `parse_status`.
- `docs/sample.svg`: the README's colored preview. Update it if the output format changes.

## Conventions & Constraints

- The output format is pinned by the golden test `full_input_matches_exact_output` in `render.rs`. Change the format deliberately and update the test, README and `docs/sample.svg` together.
- Startup speed is the point of the project. Avoid adding dependencies or work that runs on every invocation.
- Code must build and behave the same on all three OSes. Platform differences go behind `cfg!(windows)` / `#[cfg(windows)]` (home folder variable, case-insensitive paths, `CREATE_NO_WINDOW`).
- **Versioning is CalVer `YYYY.MM.DD`** (UTC, with a `.N` suffix for repeated same-day releases). `Cargo.toml` stays at `0.0.0`; the release workflow injects the version through `STATUSLINE_VERSION`.
- **Branch flow**: all changes go through pull requests into `master`. Never commit or push to `master` directly. Branches are named `<type>/short-description` (`fix/...`, `feat/...`, `ci/...`), and PRs are opened as drafts. CI (`.github/workflows/ci.yml`) runs on PRs and on pushes to `master`.
- **PRs are usually rebase-merged**, so their commits land on `master` with new hashes and the PR branch's own commits never become ancestors of `master`:
  - Start every branch from a freshly fetched `origin/master` (`git fetch --prune` first), never from another PR's branch or a merged one. Avoid stacked PRs.
  - If a PR must be stacked, then once its base PR merges, replay only its own commits with `git rebase --onto origin/master <old base tip>` and push with `--force-with-lease`. GitHub retargets the PR to `master`, but it shows conflicts until this is done.
  - A merged local branch can't be removed with `git branch -d`, because git doesn't see it as merged. Check the PR is merged, then use `-D`.
- **Releases** are manual and only from `master`: run the **Release** workflow from the Actions tab. It computes the version, builds all four targets, tags `vYYYY.MM.DD` and publishes the GitHub release with auto-generated notes. Afterwards, move the `[Unreleased]` changelog entries under a heading for the new version.
- `CHANGELOG.md` follows Keep a Changelog; add user-visible changes under `[Unreleased]`.
- **Attribution**: never add a `Co-Authored-By: Claude` trailer to commits. PR descriptions and PR comments written by Claude may carry a marker (e.g. a "🤖 Generated with Claude Code" footer) so they can be told apart from ones written by a human.
