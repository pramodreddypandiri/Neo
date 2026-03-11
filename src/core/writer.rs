/// core/writer.rs
///
/// Serializes a Neo struct to neo.md format.
///
/// Design principles:
///   - Token efficiency first — every character costs
///   - Human readable second — developers need to read/verify it
///   - Machine parseable — coding agents need to extract info fast
///
/// Format is intentionally not JSON/YAML to avoid syntax overhead.
/// A flat, pipe-delimited format is both compact and LLM-friendly.

use std::path::Path;
use crate::types::{Neo, NeoError};

/// Threshold for including a file in the HOTSPOTS section.
/// Files with fewer dependents than this are not hotspots.
const HOTSPOT_THRESHOLD: u32 = 3;

/// Writes a Neo struct to neo.md at the given path.
///
/// Overwrites any existing neo.md file.
/// Creates the file if it doesn't exist.
pub fn write_neo(neo: &Neo, output_path: &Path) -> Result<(), NeoError> {
    let content = render_neo(neo);
    std::fs::write(output_path, content)?;
    Ok(())
}

/// Renders a Neo struct to a neo.md string.
///
/// Separated from write_neo for testability —
/// we can test the rendered output without touching the filesystem.
pub fn render_neo(neo: &Neo) -> String {
    let mut out = String::new();

    // ── Header ──────────────────────────────────────────────────────────
    // Metadata line is compact — only what coding agents need to know
    out.push_str(&format!(
        "# NEO\ngenerated: {} | files: {} | language: {} | version: {}\n",
        neo.config.generated_at,
        neo.config.file_count,
        neo.config.language,
        neo.config.version,
    ));

    // ── Structure ────────────────────────────────────────────────────────
    // One line per file: path | purpose | deps
    // Sorted alphabetically so diffs are stable across regenerations
    out.push_str("\n## STRUCTURE\n");

    let mut sorted_files = neo.files.clone();
    sorted_files.sort_by(|a, b| a.path.cmp(&b.path));

    for file in &sorted_files {
        // Format deps as comma-separated list
        // Only include deps if there are any (saves space)
        if file.deps.is_empty() {
            out.push_str(&format!(
                "{} | {}\n",
                file.path,
                file.purpose,
            ));
        } else {
            out.push_str(&format!(
                "{} | {} | deps: {}\n",
                file.path,
                file.purpose,
                file.deps.join(", "),
            ));
        }
    }

    // ── Conventions ──────────────────────────────────────────────────────
    // Only write confirmed conventions — unconfirmed ones are noise
    let confirmed: Vec<_> = neo.conventions.iter()
        .filter(|c| c.confirmed)
        .collect();

    if !confirmed.is_empty() {
        out.push_str("\n## CONVENTIONS\n");
        for conv in confirmed {
            out.push_str(&format!("{}: {}\n", conv.key, conv.value));
        }
    }

    // ── Hotspots ─────────────────────────────────────────────────────────
    // Only write files that exceed the hotspot threshold
    // Sorted by hotspot score descending (most depended-on first)
    let hotspots = neo.hotspots();
    let significant_hotspots: Vec<_> = hotspots.iter()
        .filter(|f| f.hotspot_score >= HOTSPOT_THRESHOLD)
        .collect();

    if !significant_hotspots.is_empty() {
        out.push_str("\n## HOTSPOTS\n");
        for file in significant_hotspots {
            out.push_str(&format!(
                "{} | {} dependents | edit carefully\n",
                file.path,
                file.hotspot_score,
            ));
        }
    }

    // ── Entry Points ─────────────────────────────────────────────────────
    if !neo.entry_points.is_empty() {
        out.push_str("\n## ENTRY POINTS\n");
        for ep in &neo.entry_points {
            out.push_str(&format!("{}: {}\n", ep.role, ep.path));
        }
    }

    out
}

/// Reads and parses an existing neo.md file.
///
/// Used by `neo update` and `neo validate` to load current state.
/// Returns NeoError::NotInitialized if neo.md doesn't exist.
pub fn read_neo(neo_path: &Path) -> Result<Neo, NeoError> {
    if !neo_path.exists() {
        return Err(NeoError::NotInitialized);
    }

    let content = std::fs::read_to_string(neo_path)?;
    parse_neo(&content)
}

/// Parses neo.md content string into a Neo struct.
///
/// This is the inverse of render_neo.
/// Parsing is lenient — unknown lines are silently skipped.
fn parse_neo(content: &str) -> Result<Neo, NeoError> {
    use crate::types::{NeoConfig, NeoConvention, NeoFile, EntryPoint};

    let mut neo = Neo::new("unknown".to_string());
    let mut current_section = "";

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and the header line
        if line.is_empty() || line == "# NEO" {
            continue;
        }

        // Parse metadata line
        // Format: "generated: X | files: Y | language: Z | version: W"
        if line.starts_with("generated:") {
            let parts: Vec<&str> = line.split('|').collect();
            for part in parts {
                let kv: Vec<&str> = part.splitn(2, ':').collect();
                if kv.len() == 2 {
                    match kv[0].trim() {
                        "generated" => neo.config.generated_at = kv[1].trim().to_string(),
                        "language" => neo.config.language = kv[1].trim().to_string(),
                        "files" => {
                            neo.config.file_count = kv[1].trim().parse().unwrap_or(0)
                        }
                        "version" => {
                            neo.config.version = kv[1].trim().parse().unwrap_or(1)
                        }
                        _ => {}
                    }
                }
            }
            continue;
        }

        // Detect section headers
        if line.starts_with("## ") {
            current_section = match line {
                "## STRUCTURE" => "structure",
                "## CONVENTIONS" => "conventions",
                "## HOTSPOTS" => "hotspots",
                "## ENTRY POINTS" => "entry_points",
                _ => "",
            };
            continue;
        }

        // Parse section content
        match current_section {
            "structure" => {
                // Format: "path | purpose | deps: a, b, c"
                // or:     "path | purpose"
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                if parts.len() >= 2 {
                    let path = parts[0].trim().to_string();
                    let purpose = parts[1].trim().to_string();
                    let deps = if parts.len() == 3 {
                        let dep_str = parts[2].trim().strip_prefix("deps:").unwrap_or("").trim();
                        dep_str.split(',').map(|d| d.trim().to_string()).collect()
                    } else {
                        Vec::new()
                    };

                    neo.files.push(NeoFile {
                        path,
                        purpose,
                        deps,
                        dependents: Vec::new(),
                        hotspot_score: 0,
                    });
                }
            }
            "conventions" => {
                // Format: "key: value"
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    neo.conventions.push(NeoConvention {
                        key: parts[0].trim().to_string(),
                        value: parts[1].trim().to_string(),
                        confidence: 1.0, // loaded conventions are assumed confirmed
                        confirmed: true,
                    });
                }
            }
            "entry_points" => {
                // Format: "role: path"
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    neo.entry_points.push(EntryPoint {
                        role: parts[0].trim().to_string(),
                        path: parts[1].trim().to_string(),
                    });
                }
            }
            // HOTSPOTS section is derived data — skip when reading
            // It gets recalculated from the graph on next update
            _ => {}
        }
    }

    Ok(neo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NeoConvention, NeoFile, EntryPoint};

    fn sample_neo() -> Neo {
        let mut neo = Neo::new("typescript".to_string());
        neo.files.push(NeoFile {
            path: "src/auth/token.ts".to_string(),
            purpose: "JWT creation and validation".to_string(),
            deps: vec!["src/config/env.ts".to_string()],
            dependents: vec!["src/api/user.ts".to_string()],
            hotspot_score: 1,
        });
        neo.conventions.push(NeoConvention {
            key: "error-handling".to_string(),
            value: "AppError class".to_string(),
            confidence: 0.9,
            confirmed: true,
        });
        neo.entry_points.push(EntryPoint {
            role: "app-entry".to_string(),
            path: "index.ts".to_string(),
        });
        neo
    }

    #[test]
    fn test_render_contains_file() {
        let neo = sample_neo();
        let rendered = render_neo(&neo);
        assert!(rendered.contains("src/auth/token.ts"));
        assert!(rendered.contains("JWT creation and validation"));
    }

    #[test]
    fn test_render_contains_convention() {
        let neo = sample_neo();
        let rendered = render_neo(&neo);
        assert!(rendered.contains("error-handling: AppError class"));
    }

    #[test]
    fn test_roundtrip() {
        let neo = sample_neo();
        let rendered = render_neo(&neo);
        let parsed = parse_neo(&rendered).unwrap();

        assert_eq!(parsed.config.language, "typescript");
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].path, "src/auth/token.ts");
        assert_eq!(parsed.conventions.len(), 1);
        assert_eq!(parsed.entry_points.len(), 1);
    }
}
