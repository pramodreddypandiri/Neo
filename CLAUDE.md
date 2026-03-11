# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Neo is a Rust CLI tool + library that generates `neo.md` — a compact, pipe-delimited codebase map for AI coding agents. The map lets agents navigate directly to files (~4,000 tokens once) instead of grepping the codebase repeatedly (5,000–7,500 tokens per task).

## Commands

```bash
# Build
cargo build
cargo build --release          # Binary at: target/release/neo

# Run
cargo run -- init --agent claude              # Full scan: generate neo.md + CLAUDE.md
cargo run -- init --agent all                 # Generate all supported agent files
cargo run -- update src/core/graph.rs         # Incremental update for changed files
cargo run -- validate                         # CI/CD sync check

# Test
cargo test --lib                              # All library tests
cargo test --lib parser                       # Parser module tests only
cargo test --lib core::writer                 # Specific module

# Format / Lint
cargo fmt
cargo clippy
```

**Required env var:** `ANTHROPIC_API_KEY` — needed for `neo init` and `neo update`.

## Architecture

The project is split into a binary (`src/main.rs` → `neo` CLI) and a library (`src/lib.rs` → `neo_core`). The CLI layer only owns `cli/`; everything else is consumed via `neo_core::`.

```
src/
├── main.rs           CLI entry point — parses args, dispatches to cli/
├── lib.rs            Library entry point — declares all shared modules
├── types.rs          ALL shared types (single source of truth)
│
├── core/
│   ├── scanner.rs    Walks directory tree, returns indexable source files
│   ├── graph.rs      Builds reverse dependency graph + computes hotspot scores
│   └── writer.rs     Reads/writes neo.md (parse + render, pipe-delimited format)
│
├── parser/
│   ├── mod.rs        Parser trait + get_parser_for_file() dispatcher
│   ├── detector.rs   Auto-detects language from project root markers
│   ├── typescript.rs TypeScript/TSX import + export extraction
│   └── python.rs     Python import + export extraction
│
├── ai/
│   ├── summarizer.rs Batched AI summarization — 10 files/call, claude-haiku-4-5-20251001
│   └── conventions.rs AI convention inference with confidence scores
│
├── agent/
│   └── claude.rs     Generates CLAUDE.md (idempotent)
│
└── cli/
    ├── init.rs       `neo init` — full scan, orchestrates all modules
    ├── update.rs     `neo update` — re-parses/re-summarizes changed files
    └── validate.rs   `neo validate` — ghost file errors, missing file warnings
```

## Key Rules

**Types:** All shared types live only in `types.rs`. `NeoError` uses `thiserror` (library layer); CLI layer uses `anyhow`.

**Module ownership:** `cli/` imports use `neo_core::`, never `crate::`.

**AI calls:** Only `ai/summarizer.rs` and `ai/conventions.rs` make Anthropic API calls. Model is always `claude-haiku-4-5-20251001`. Batch size is 10 files per call.

**neo.md format:** Pipe-delimited flat text (not JSON/YAML). Sections: STRUCTURE, CONVENTIONS, HOTSPOTS, ENTRY POINTS. Hotspot threshold: 3+ dependents. Always sorted alphabetically for stable diffs. Never edit manually.

**Parser extensibility:** Adding a language = implement the `Parser` trait → register in `parser/mod.rs → get_parser_for_file()` → add detection marker in `detector.rs` → add entry point candidates in `cli/init.rs`. Tests go in `#[cfg(test)]` in the same file.

**Rust version constraint:** Do not add dependencies requiring rustc > 1.75 without verifying compatibility.
