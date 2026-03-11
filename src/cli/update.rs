/// cli/update.rs
///
/// Implementation of `neo update <files...>`.
///
/// Called by the coding agent after making file changes.
/// Updates only the affected entries in neo.md — not a full rescan.
///
/// This is the most frequently called Neo command.
/// It must be fast: agent calls it after every task.
///
/// Steps:
///   1. Load existing neo.md
///   2. For each changed file:
///      a. Re-parse deps and exports
///      b. Re-generate AI summary
///      c. Update entry in Neo
///   3. Rebuild hotspot scores (affected by dep changes)
///   4. Write updated neo.md

use std::path::PathBuf;
use colored::*;
use crate::core::{graph, writer};
use crate::parser;
use crate::ai::summarizer::{self, FileToSummarize};
use crate::types::NeoFile;

/// Runs `neo update` for the given list of changed files.
///
/// `files` — list of relative paths that changed
///   e.g. ["src/auth/token.ts", "src/api/user.ts"]
pub async fn run(project_root: PathBuf, files: Vec<String>) -> Result<(), anyhow::Error> {
    if files.is_empty() {
        println!("{}", "  No files specified. Usage: neo update <file1> <file2>".yellow());
        return Ok(());
    }

    println!(
        "  {} Updating Neo for {} file(s)...",
        "→".dimmed(),
        files.len()
    );

    // ── Load existing Neo ────────────────────────────────────────────────
    let neo_path = project_root.join("neo.md");
    let mut neo = writer::read_neo(&neo_path)?;

    // ── Get API key ──────────────────────────────────────────────────────
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        anyhow::anyhow!("ANTHROPIC_API_KEY environment variable not set.")
    })?;

    // ── Process each changed file ────────────────────────────────────────
    let mut files_to_summarize: Vec<FileToSummarize> = Vec::new();
    let mut files_to_remove: Vec<String> = Vec::new();

    for relative_path in &files {
        let absolute_path = project_root.join(relative_path);

        if !absolute_path.exists() {
            // File was deleted — remove from Neo
            println!("  {} Removing deleted file: {}", "−".red(), relative_path);
            files_to_remove.push(relative_path.clone());
            continue;
        }

        // Re-parse deps and exports
        let (deps, exports) = if let Some(p) = parser::get_parser_for_file(&absolute_path) {
            let deps = p.extract_deps(&absolute_path, &project_root).unwrap_or_default();
            let exports = p.extract_exports(&absolute_path);
            (deps, exports)
        } else {
            (Vec::new(), Vec::new())
        };

        // Read content preview for re-summarization
        let content_preview = summarizer::read_content_preview(&absolute_path, 500);

        files_to_summarize.push(FileToSummarize {
            path: relative_path.clone(),
            content_preview,
            exports,
        });

        // Update the NeoFile entry (purpose will be updated after AI call)
        neo.upsert_file(NeoFile {
            path: relative_path.clone(),
            purpose: String::new(), // Filled below
            deps,
            dependents: Vec::new(), // Rebuilt by graph
            hotspot_score: 0,       // Rebuilt by graph
        });
    }

    // ── Remove deleted files ─────────────────────────────────────────────
    for path in files_to_remove {
        neo.remove_file(&path);
    }

    // ── Re-summarize changed files ───────────────────────────────────────
    if !files_to_summarize.is_empty() {
        let summaries = summarizer::summarize_files(files_to_summarize, &api_key).await?;

        for (path, purpose) in summaries {
            if let Some(file) = neo.files.iter_mut().find(|f| f.path == path) {
                file.purpose = purpose;
            }
        }
    }

    // ── Rebuild dependency graph ─────────────────────────────────────────
    // Must rebuild because dep changes ripple through the whole graph
    // Reset all reverse deps first, then rebuild from scratch
    for file in neo.files.iter_mut() {
        file.dependents.clear();
        file.hotspot_score = 0;
    }
    graph::build_reverse_deps(&mut neo.files);

    // ── Write updated Neo ────────────────────────────────────────────────
    writer::write_neo(&neo, &neo_path)?;

    println!(
        "  {} Neo updated ({} file(s) processed)",
        "✓".green(),
        files.len()
    );

    Ok(())
}
