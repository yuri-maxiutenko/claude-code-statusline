# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Calendar Versioning](https://calver.org/) (`YYYY.MM.DD`, the UTC release date).

## [Unreleased]

### Fixed
- The cost is rounded to cents on its decimal value, half away from zero, so `0.145` shows as `$0.15` and `0.125` as `$0.13` (both were rounded down before).
- A file or branch name that isn't valid UTF-8 no longer hides the git branch.
- The home folder is found even when `HOME` / `USERPROFILE` is unset, so the `~` shortening still works.
- A non-Unicode path to the binary no longer crashes it.
- Control characters in the directory, branch, model or output style name are dropped, so they can't inject terminal escape sequences into the status line.

### Changed
- Clippy runs on Windows, Linux and macOS in CI. The Release workflow runs only from `master` and tests the static Linux (musl) build it ships.

## [2026.10.25]

### Added
- Status line showing the model and effort level, output style, a context usage bar, session cost, Claude.ai rate limits, the working directory and the git branch with a changes marker.
- Native builds for Windows x64, Linux x64 (static musl, works in WSL), and macOS on Apple silicon and Intel.
- CI running formatting, Clippy and tests on Windows, Linux and macOS, and a manually triggered release workflow.
