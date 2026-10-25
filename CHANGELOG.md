# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Calendar Versioning](https://calver.org/) (`YYYY.MM.DD`, the UTC release date).

## [Unreleased]

### Added
- Status line showing the model and effort level, output style, a context usage bar, session cost, Claude.ai rate limits, the working directory and the git branch with a changes marker.
- Native builds for Windows x64, Linux x64 (static musl, works in WSL), and macOS on Apple silicon and Intel.
- CI running formatting, Clippy and tests on Windows, Linux and macOS, and a manually triggered release workflow.
