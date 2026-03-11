# Neo — Architecture

## What Neo Does

Neo generates `neo.md` — a compact, AI-native codebase map that coding agents
read instead of grepping the codebase. One-time generation, incremental updates,
significant token savings per session.

```
Without Neo:  agent greps 10-15 files × 500 tokens = 5,000–7,500 tokens/task
With Neo:     agent reads neo.md once = ~4,000 tokens, navigates directly
```

---

## High-Level Design

```
┌─────────────────────────────────────────────────────┐
│                     CLI Layer                        │
│         neo init │ neo update │ neo validate         │
└────────────────────────┬────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────┐
│                    Core Engine                       │
│         scanner → parser → graph → writer            │
└──────┬──────────────┬──────────────────┬────────────┘
       │              │                  │
┌──────▼──────┐ ┌─────▼──────┐ ┌────────▼───────┐
│  AI Layer   │ │Parser Layer│ │  Agent Layer   │
│  summarizer │ │  ts/py/... │ │  CLAUDE.md     │
│  conventions│ │  detector  │ │  .cursorrules  │
└─────────────┘ └────────────┘ └────────────────┘
                         │
                    ┌────▼────┐
                    │ neo.md  │
                    └─────────┘
```

---

## Module Responsibilities

### `types.rs` — Shared Data Structures
Single source of truth for all types. Every module imports from here.

| Type | Purpose |
|---|---|
| `Neo` | The complete index — config + files + conventions + entry points |
| `NeoFile` | One file: path, purpose, deps, dependents, hotspot_score |
| `NeoConfig` | Index metadata: language, timestamp, version |
| `NeoConvention` | A detected pattern: key, value, confidence, confirmed |
| `EntryPoint` | A named entry point: role + path |
| `NeoError` | Typed errors for the library layer (thiserror) |

---

### `core/scanner.rs` — File Discovery
Walks the project directory. Returns only indexable source files.

**Always excluded:**
- `node_modules`, `.git`, `dist`, `build`, `target`, `__pycache__`
- `.venv`, `vendor`, `.next`, `.expo`
- React Native: `android/build`, `ios/build`, `assets`
- File types: images, fonts, binaries, lock files, dotfiles
- Test files: `*.test.*`, `*.spec.*`

**Input:** project root path
**Output:** `ScanResult { files: Vec<PathBuf>, skipped: u32 }`

---

### `core/graph.rs` — Dependency Graph
Builds reverse dependencies after all files are parsed.

```
Forward deps  (A imports B) → stored in NeoFile.deps by parsers
Reverse deps  (B is imported by A) → computed here, stored in NeoFile.dependents
Hotspot score → len(dependents), cached in NeoFile.hotspot_score
```

Algorithm: O(n × avg_deps) — linear in practice.
Runs after full parse. Also runs on every `neo update` (full rebuild from deps).

---

### `core/writer.rs` — neo.md Serialization

**Render** (`Neo → String → neo.md`):
```
# NEO
generated: <ts> | files: <n> | language: <lang> | version: 1

## STRUCTURE
<path> | <purpose> | deps: <a>, <b>

## CONVENTIONS
<key>: <value>

## HOTSPOTS
<path> | <n> dependents | edit carefully

## ENTRY POINTS
<role>: <path>
```

**Parse** (`neo.md → Neo`):
- Lenient — unknown lines silently skipped
- HOTSPOTS section is skipped on read (recomputed from graph)
- Loaded conventions are treated as confirmed (confidence: 1.0)

Format choice: pipe-delimited flat text (not JSON/YAML).
Reason: avoids `{}[]:"` syntax overhead — every character costs tokens.

---

### `parser/mod.rs` — Parser Trait (Language Abstraction)

```rust
pub trait Parser: Send + Sync {
    fn can_parse(&self, filepath: &Path) -> bool;
    fn extract_deps(&self, filepath: &Path, project_root: &Path) -> Result<Vec<String>, NeoError>;
    fn extract_exports(&self, filepath: &Path) -> Vec<String>;
    fn extensions(&self) -> Vec<&'static str>;
    fn language_name(&self) -> &'static str;
}
```

Core engine never knows the language. Parsers are swapped via `get_parser_for_file()`.
Current implementations: TypeScript, Python.
Adding a language = implementing this trait. Nothing else changes.

---

### `parser/detector.rs` — Language Detection
Checks project root for well-known marker files.

```
tsconfig.json + package.json  → typescript
package.json                  → javascript
requirements.txt / pyproject  → python
Cargo.toml                    → rust
go.mod                        → go
pom.xml / build.gradle        → java
Gemfile                       → ruby
mix.exs                       → elixir
```

---

### `ai/summarizer.rs` — File Summarization
The only module that makes AI API calls during `neo init` and `neo update`.

**Strategy:**
- Batch 10 files per API call (reduces per-file prompt overhead by ~60%)
- Send first 400 chars of each file + known exports as context
- Prompt forces one-line output (max 10 words)
- Model: `claude-haiku-4-5-20251001` — fastest, cheapest
- Fallback: filename-based description if AI call fails

**Token math per batch:**
```
10 files × 400 chars preview  = ~1,000 tokens input
prompt overhead               = ~200 tokens
10 one-line summaries output  = ~150 tokens
Total per batch               = ~1,350 tokens for 10 files
vs. 10 separate calls         = ~10 × 600 = 6,000 tokens
Saving: ~78% via batching
```

---

### `ai/conventions.rs` — Convention Inference
Runs once during `neo init`. Samples 20 files, asks AI to detect patterns.

**Output format from AI:**
```
error-handling: AppError class [confidence: 0.91]
state-management: Redux Toolkit [confidence: 0.88]
```

**Flow:**
1. Sample 20 files evenly distributed across codebase
2. Send to AI with structured prompt
3. Parse response into `Vec<NeoConvention>` (confirmed: false)
4. Show to user for confirmation
5. Only confirmed conventions written to neo.md

---

### `agent/claude.rs` — CLAUDE.md Generation
Injects Neo usage instructions into CLAUDE.md.

**Behavior:**
- No CLAUDE.md → creates it with Neo block
- CLAUDE.md exists, no Neo block → prepends Neo block
- CLAUDE.md exists, has Neo block → replaces block in place
- Idempotent — safe to run multiple times

Neo block is placed at the TOP of CLAUDE.md so the agent reads it first.

---

### `cli/init.rs` — Full Initialization Flow

```
1.  detect_language()           → language string
2.  scan_project()              → Vec<PathBuf>
3.  for each file:
      get_parser_for_file()     → deps, exports
      read_content_preview()    → first 500 chars
4.  build_reverse_deps()        → dependents, hotspot_score
5.  summarize_files()           → AI: one-line purpose per file
6.  infer_conventions()         → AI: detected patterns
7.  user confirms conventions
8.  detect_entry_points()       → known entry files
9.  write_neo()                 → neo.md
10. write_claude_instructions() → CLAUDE.md
```

---

### `cli/update.rs` — Incremental Update Flow

Called by coding agent after every file change.

```
1. read_neo()                   → load existing Neo
2. for each changed file:
     if deleted → remove_file()
     if exists  → re-parse deps + re-summarize
3. rebuild_reverse_deps()       → full graph rebuild
4. write_neo()                  → write updated Neo
```

Graph is fully rebuilt on every update because dep changes
can ripple through the entire reverse dependency tree.

---

### `cli/validate.rs` — Sync Checking

Used in CI/CD. Compares neo.md against actual filesystem.

```
Checks:
  Files in neo.md that no longer exist on disk  → ghost files (error)
  Files on disk missing from neo.md             → missing files (warning)

Exit code 0 → in sync
Exit code 1 → stale (CI should fail the PR)
```

---

## Data Flow: `neo init`

```
Filesystem
    │
    ▼
scanner.rs ──────────────────────── Vec<PathBuf>
    │
    ▼
parser/*.rs ─────────────────────── Vec<NeoFile> (deps, exports; no purpose yet)
    │
    ▼
graph.rs ────────────────────────── fills NeoFile.dependents + hotspot_score
    │
    ▼
ai/summarizer.rs ────────────────── fills NeoFile.purpose (batched API calls)
    │
    ▼
ai/conventions.rs ───────────────── Vec<NeoConvention> (user confirms)
    │
    ▼
Neo { config, files, conventions, entry_points }
    │
    ▼
core/writer.rs ──────────────────── neo.md
    │
    ▼
agent/claude.rs ─────────────────── CLAUDE.md
```

---

## Binary vs Library Split

```
Cargo.toml declares both:
  [[bin]] name = "neo"      path = "src/main.rs"
  [lib]   name = "neo_core" path = "src/lib.rs"

lib.rs  → owns: types, core, parser, ai, agent
main.rs → owns: cli only
            imports everything via `use neo_core::*`

cli/*.rs → use neo_core:: (not crate::)
```

This allows external tools to import `neo_core` and use the
parsing/graph/writing engine without going through the CLI.

---

## neo.md Token Budget

Target: fit within 4,000 tokens regardless of codebase size.

```
Per file entry:    ~20 tokens
200 file codebase: ~4,000 tokens  ← target ceiling
500 file codebase: ~10,000 tokens ← needs summarization strategy
```

For large codebases (500+ files), future strategy:
- Summarize at directory level instead of file level
- Only index files touched in last 90 days
- Configurable depth limit

---

## Planned: Not Yet Built

| Feature | Location | Notes |
|---|---|---|
| Git hook installer | `hooks/git.rs` | pre-commit safety net |
| Cursor agent support | `agent/cursor.rs` | `.cursorrules` generation |
| Go parser | `parser/go.rs` | implement Parser trait |
| Rust parser | `parser/rust.rs` | implement Parser trait |
| npm distribution | `npm/` | wraps binary for JS devs |
| Integration tests | `tests/fixtures/` | sample codebases per language |
| Config file | `neo.toml` | custom excludes, hotspot threshold |
