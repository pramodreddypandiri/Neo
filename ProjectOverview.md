# CLAUDE.md — Neo

## NEO — Codebase Map (READ FIRST)

A `neo.md` file will exist in the project root once `neo init` has been run.

**ALWAYS before searching the codebase:**
1. Read `neo.md` first
2. Use it to locate files directly
3. Only grep if `neo.md` doesn't have the answer

**ALWAYS after making file changes:**
Run: `neo update <space-separated list of files you changed>`
Example: `neo update src/core/graph.rs src/types.rs`

**NEVER:**
- Grep the codebase for something neo.md already maps
- Skip updating neo.md after changes
- Edit neo.md manually (it is auto-generated)

---

## Project Overview

Neo is a CLI tool written in Rust that generates a compact `neo.md` codebase map
for coding agents. The map lets agents navigate directly to files instead of
grepping the codebase — saving significant tokens per session.

---

## Build & Run

```bash
# Build
cargo build

# Run all tests
cargo test --lib

# Run a command
cargo run -- init --agent claude
cargo run -- update src/core/graph.rs
cargo run -- validate

# Release build
cargo build --release
# Binary at: target/release/neo
```

---

## Project Structure

```
src/
├── main.rs           CLI entry point — parses args, dispatches to cli/
├── lib.rs            Library entry point — declares all shared modules
├── types.rs          ALL shared types — import from here only
│
├── core/
│   ├── scanner.rs    Walks directory tree, returns indexable files
│   ├── graph.rs      Builds reverse dependency graph + hotspot scores
│   └── writer.rs     Reads/writes neo.md (parse + render)
│
├── parser/
│   ├── mod.rs        Parser trait definition — language-agnostic interface
│   ├── detector.rs   Auto-detects language from project root markers
│   ├── typescript.rs TypeScript/TSX import + export extraction
│   └── python.rs     Python import + export extraction
│
├── ai/
│   ├── summarizer.rs Batched AI file summarization (Anthropic API)
│   └── conventions.rs AI convention inference with confidence scores
│
├── agent/
│   └── claude.rs     Generates CLAUDE.md instructions (idempotent)
│
└── cli/
    ├── init.rs       `neo init` — full scan, orchestrates all modules
    ├── update.rs     `neo update` — incremental update for changed files
    └── validate.rs   `neo validate` — CI/CD sync checker
```

---

## Key Architecture Rules

**Module ownership:**
- `lib.rs` owns all shared modules (types, core, parser, ai, agent)
- `main.rs` owns only `cli/` — everything else comes from `neo_core::`
- `cli/` imports use `neo_core::` not `crate::` — they consume the library

**Types:**
- All shared types live in `types.rs` — never define types elsewhere
- `NeoError` uses `thiserror` (library layer)
- CLI layer uses `anyhow` for error display

**Parser trait:**
- Adding a language = implementing `Parser` trait in `parser/`
- Register the new parser in `parser/mod.rs → get_parser_for_file()`
- Core engine never knows what language it's dealing with

**AI calls:**
- Only `ai/summarizer.rs` and `ai/conventions.rs` make API calls
- Model: `claude-haiku-4-5-20251001` (fast + cheap for this task)
- Batch size: 10 files per API call
- API key: `ANTHROPIC_API_KEY` env var — never hardcode

**neo.md format:**
- Pipe-delimited flat text — not JSON/YAML (avoids syntax token overhead)
- Sections: STRUCTURE, CONVENTIONS, HOTSPOTS, ENTRY POINTS
- Hotspot threshold: files with 3+ dependents
- Always sorted alphabetically so git diffs are stable

---

## Adding a New Language Parser

1. Create `src/parser/<language>.rs`
2. Implement the `Parser` trait:
   - `can_parse()` — check file extension
   - `extract_deps()` — parse import statements, return relative paths
   - `extract_exports()` — parse exported symbols
   - `extensions()` — return handled extensions
   - `language_name()` — return display name
3. Register in `src/parser/mod.rs → get_parser_for_file()`
4. Add detection marker in `src/parser/detector.rs → detect_language()`
5. Add entry point candidates in `src/cli/init.rs → detect_entry_points()`
6. Write tests in the same file under `#[cfg(test)]`

---

## Environment Variables

```
ANTHROPIC_API_KEY   Required for neo init and neo update (AI summarization)
```

---

## Testing

Tests live alongside the code in `#[cfg(test)]` blocks.

```bash
cargo test --lib                    # all library tests
cargo test --lib parser             # parser tests only
cargo test --lib core::writer       # specific module
```

Key test fixtures in tests/fixtures/ (coming soon):
- `typescript/` — sample TS project for integration tests
- `python/`     — sample Python project
- `rust/`       — sample Rust project

---

## What NOT to do

- Do not edit `neo.md` manually — it is always regenerated
- Do not add dependencies without checking Rust version compatibility
  (current environment: rustc 1.75 — some newer crates require 1.82+)
- Do not make AI API calls outside of `ai/` module
- Do not use `crate::` in `cli/` — use `neo_core::` instead
- Do not define types outside of `types.rs`
