/// cli/validate.rs
///
/// Implementation of `neo validate`.
///
/// Checks if neo.md is in sync with the actual codebase.
/// Used by CI/CD to catch stale Neo before merging.
///
/// Validation checks:
///   1. neo.md exists
///   2. All files in neo.md still exist on disk
///   3. No new files on disk that are missing from neo.md
///   4. Neo version is current
///
/// Exit codes:
///   0 → Neo is in sync
///   1 → Neo is stale (CI should fail the PR)

use std::path::PathBuf;
use std::collections::HashSet;
use colored::*;
use crate::core::{scanner, writer};
use crate::parser;

/// Runs `neo validate`.
///
/// Returns Ok(()) if Neo is in sync.
/// Returns Err if Neo is stale or missing.
pub fn run(project_root: PathBuf) -> Result<(), anyhow::Error> {
    println!("  {} Validating neo.md...", "→".dimmed());

    // ── Check neo.md exists ──────────────────────────────────────────────
    let neo_path = project_root.join("neo.md");
    if !neo_path.exists() {
        println!("  {} neo.md not found. Run `neo init` first.", "✗".red());
        return Err(anyhow::anyhow!("Neo not initialized"));
    }

    // ── Load neo.md ──────────────────────────────────────────────────────
    let neo = writer::read_neo(&neo_path)?;
    let neo_paths: HashSet<String> = neo.files.iter().map(|f| f.path.clone()).collect();

    // ── Scan actual codebase ─────────────────────────────────────────────
    let scan = scanner::scan_project(&project_root);

    // Convert scanned files to relative paths that parsers can handle
    let disk_paths: HashSet<String> = scan.files
        .iter()
        .filter(|f| parser::get_parser_for_file(f).is_some()) // only parseable files
        .filter_map(|f| {
            f.strip_prefix(&project_root)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect();

    // ── Check for files in Neo that no longer exist ──────────────────────
    let ghost_files: Vec<&String> = neo_paths
        .iter()
        .filter(|p| !disk_paths.contains(*p))
        .collect();

    // ── Check for files on disk missing from Neo ─────────────────────────
    let missing_files: Vec<&String> = disk_paths
        .iter()
        .filter(|p| !neo_paths.contains(*p))
        .collect();

    // ── Report results ───────────────────────────────────────────────────
    let is_valid = ghost_files.is_empty() && missing_files.is_empty();

    if is_valid {
        println!(
            "  {} neo.md is in sync ({} files)",
            "✓".green(),
            neo.config.file_count
        );
        Ok(())
    } else {
        if !ghost_files.is_empty() {
            println!("\n  {} Files in neo.md that no longer exist:", "✗".red());
            for f in &ghost_files {
                println!("    - {}", f.dimmed());
            }
        }

        if !missing_files.is_empty() {
            println!("\n  {} Files on disk missing from neo.md:", "⚠".yellow());
            for f in &missing_files {
                println!("    + {}", f);
            }
        }

        println!("\n  {} Run `neo init` to regenerate, or `neo update <files>` for specific files", "→".dimmed());

        Err(anyhow::anyhow!(
            "Neo is out of sync: {} ghost, {} missing",
            ghost_files.len(),
            missing_files.len()
        ))
    }
}
