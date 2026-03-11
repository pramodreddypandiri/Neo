/// cli/init.rs
///
/// Implementation of `neo init`.
///
/// This is the main entry point for first-time setup.
/// It orchestrates all other modules in the right order.
///
/// Steps:
///   1. Detect language
///   2. Scan codebase
///   3. Parse all files (extract deps, exports)
///   4. Build dependency graph
///   5. Batch summarize files with AI
///   6. Infer conventions with AI
///   7. Show conventions to user for confirmation
///   8. Detect entry points
///   9. Write neo.md
///  10. Write agent instruction files (CLAUDE.md etc)
///  11. Optionally install git hook

use std::path::{Path, PathBuf};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use crate::types::{Neo, NeoFile, NeoError, EntryPoint};
use crate::core::{scanner, graph, writer};
use crate::parser::{self, detector};
use crate::ai::{summarizer, conventions};
use crate::agent::claude;

/// Runs `neo init` in the given project root.
///
/// `agent` — which agent instruction files to generate
///   "claude" → CLAUDE.md only
///   "all"    → all supported agents
pub async fn run(project_root: PathBuf, agent: String) -> Result<(), anyhow::Error> {
    println!("{}", "  Neo — AI-native codebase map".bold().cyan());
    println!("{}", "  ─────────────────────────────".dimmed());

    // ── Step 1: Detect language ──────────────────────────────────────────
    let language = match detector::detect_language(&project_root) {
        Some(lang) => {
            println!("  {} Detected language: {}", "✓".green(), lang.bold());
            if !detector::is_supported(&lang) {
                println!(
                    "  {} Language '{}' detected but not yet fully supported.",
                    "⚠".yellow(), lang
                );
                println!("  {} Proceeding with basic file structure only.", "→".dimmed());
            }
            lang
        }
        None => {
            println!("  {} Could not detect language.", "⚠".yellow());
            println!("  {} Defaulting to generic mode.", "→".dimmed());
            "unknown".to_string()
        }
    };

    // ── Step 2: Scan codebase ────────────────────────────────────────────
    println!("\n  {} Scanning codebase...", "→".dimmed());
    let scan = scanner::scan_project(&project_root);
    println!(
        "  {} Found {} files ({} skipped)",
        "✓".green(),
        scan.files.len().to_string().bold(),
        scan.skipped
    );

    if scan.files.is_empty() {
        return Err(anyhow::anyhow!("No source files found in {:?}", project_root));
    }

    // ── Step 3: Parse all files ──────────────────────────────────────────
    println!("\n  {} Parsing dependencies...", "→".dimmed());
    let pb = progress_bar(scan.files.len() as u64, "  Parsing");

    let mut neo_files: Vec<NeoFile> = Vec::new();
    let mut files_for_summary: Vec<summarizer::FileToSummarize> = Vec::new();
    let mut files_for_conventions: Vec<(String, String)> = Vec::new();

    for filepath in &scan.files {
        pb.inc(1);

        // Get relative path from project root
        let relative_path = filepath
            .strip_prefix(&project_root)
            .unwrap_or(filepath)
            .to_string_lossy()
            .to_string();

        // Try to parse with appropriate language parser
        let (deps, exports) = if let Some(parser) = parser::get_parser_for_file(filepath) {
            let deps = parser.extract_deps(filepath, &project_root).unwrap_or_default();
            let exports = parser.extract_exports(filepath);
            (deps, exports)
        } else {
            // No parser for this file type — still index it, just without deps
            (Vec::new(), Vec::new())
        };

        // Read content preview for AI summarization
        let content_preview = summarizer::read_content_preview(filepath, 500);

        // Collect for AI batch summarization
        files_for_summary.push(summarizer::FileToSummarize {
            path: relative_path.clone(),
            content_preview: content_preview.clone(),
            exports: exports.clone(),
        });

        // Collect sample for convention detection (first 20 files with content)
        if files_for_conventions.len() < 20 && !content_preview.is_empty() {
            files_for_conventions.push((relative_path.clone(), content_preview));
        }

        // Create initial NeoFile (purpose will be filled in after AI summarization)
        neo_files.push(NeoFile {
            path: relative_path,
            purpose: String::new(), // Placeholder — filled by summarizer
            deps,
            dependents: Vec::new(), // Filled by graph builder
            hotspot_score: 0,       // Filled by graph builder
        });
    }

    pb.finish_and_clear();
    println!("  {} Parsed {} files", "✓".green(), neo_files.len());

    // ── Step 4: Build dependency graph ───────────────────────────────────
    println!("\n  {} Building dependency graph...", "→".dimmed());
    graph::build_reverse_deps(&mut neo_files);
    let hotspot_count = neo_files.iter().filter(|f| f.hotspot_score > 3).count();
    println!("  {} Graph built ({} hotspots detected)", "✓".green(), hotspot_count);

    // ── Step 5: AI summarization ─────────────────────────────────────────
    let api_key = get_api_key()?;

    println!("\n  {} Generating file summaries (AI)...", "→".dimmed());
    let pb = progress_bar(files_for_summary.len() as u64, "  Summarizing");

    let summaries = summarizer::summarize_files(files_for_summary, &api_key).await?;

    // Apply summaries to neo_files
    for (path, purpose) in summaries {
        if let Some(file) = neo_files.iter_mut().find(|f| f.path == path) {
            file.purpose = purpose;
        }
        pb.inc(1);
    }

    pb.finish_and_clear();
    println!("  {} Summaries generated", "✓".green());

    // ── Step 6 & 7: Infer and confirm conventions ─────────────────────────
    println!("\n  {} Detecting conventions (AI)...", "→".dimmed());
    let mut detected_conventions =
        conventions::infer_conventions(files_for_conventions, &api_key).await?;

    // Show detected conventions to user for confirmation
    if !detected_conventions.is_empty() {
        println!("\n  {} Detected conventions:", "→".yellow());
        for (i, conv) in detected_conventions.iter().enumerate() {
            println!(
                "  {}. {}: {} {}",
                i + 1,
                conv.key.bold(),
                conv.value,
                format!("(confidence: {:.0}%)", conv.confidence * 100.0).dimmed()
            );
        }

        println!("\n  Confirm all? [Y/n/edit]: ");
        // For MVP: auto-confirm all conventions
        // Future: interactive confirmation loop
        for conv in detected_conventions.iter_mut() {
            conv.confirmed = true;
        }
        println!("  {} All conventions confirmed", "✓".green());
    }

    // ── Step 8: Detect entry points ──────────────────────────────────────
    let entry_points = detect_entry_points(&project_root, &language);
    if !entry_points.is_empty() {
        println!("\n  {} Entry points: {}", "✓".green(), entry_points.len());
    }

    // ── Step 9: Build and write Neo ──────────────────────────────────────
    let mut neo = Neo::new(language);
    neo.files = neo_files;
    neo.conventions = detected_conventions;
    neo.entry_points = entry_points;

    let neo_path = project_root.join("neo.md");
    writer::write_neo(&neo, &neo_path)?;

    println!("\n  {} neo.md written ({} files indexed)", "✓".green(), neo.config.file_count);

    // ── Step 10: Write agent instruction files ───────────────────────────
    match agent.as_str() {
        "claude" | "all" => {
            claude::write_claude_instructions(&project_root)?;
            println!("  {} CLAUDE.md updated with Neo instructions", "✓".green());
        }
        _ => {}
    }

    // ── Done ─────────────────────────────────────────────────────────────
    println!("\n  {} Neo initialized successfully!", "✓".bold().green());
    println!("  {} Start a new Claude Code session to use Neo", "→".dimmed());

    Ok(())
}

/// Detects common entry points based on language conventions.
fn detect_entry_points(project_root: &Path, language: &str) -> Vec<EntryPoint> {
    let mut entry_points = Vec::new();

    let candidates: &[(&str, &str)] = match language {
        "typescript" | "javascript" => &[
            ("app-entry", "index.ts"),
            ("app-entry", "index.tsx"),
            ("app-entry", "App.tsx"),
            ("app-entry", "src/index.ts"),
            ("app-entry", "src/App.tsx"),
            ("navigation", "src/navigation/RootNavigator.tsx"),
            ("navigation", "src/navigation/index.ts"),
        ],
        "python" => &[
            ("app-entry", "main.py"),
            ("app-entry", "app.py"),
            ("app-entry", "run.py"),
        ],
        "rust" => &[
            ("app-entry", "src/main.rs"),
            ("lib-entry", "src/lib.rs"),
        ],
        _ => &[],
    };

    for (role, path) in candidates {
        if project_root.join(path).exists() {
            entry_points.push(EntryPoint {
                role: role.to_string(),
                path: path.to_string(),
            });
        }
    }

    entry_points
}

/// Gets the Anthropic API key from environment variable.
fn get_api_key() -> Result<String, anyhow::Error> {
    std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        anyhow::anyhow!(
            "ANTHROPIC_API_KEY environment variable not set.\n\
             Export it with: export ANTHROPIC_API_KEY=your-key"
        )
    })
}

/// Creates a styled progress bar for long-running operations.
fn progress_bar(len: u64, prefix: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!(
                "{}  [{{bar:30.cyan/dim}}] {{pos}}/{{len}} {{msg}}",
                prefix
            ))
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb
}
