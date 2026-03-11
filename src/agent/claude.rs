/// agent/claude.rs
///
/// Generates CLAUDE.md instructions that tell Claude Code to use Neo.
///
/// This is the bridge between Neo and Claude Code.
/// Without this, the agent doesn't know Neo exists.
///
/// The instructions are written to be:
///   - Explicit (agents follow explicit rules reliably)
///   - Placed at the TOP of CLAUDE.md (first thing agent reads)
///   - Token-efficient (short instructions, not an essay)

use std::path::Path;
use crate::types::NeoError;

/// Neo instructions block that gets injected into CLAUDE.md.
///
/// This is placed at the top of CLAUDE.md so it's the first
/// thing the agent reads every session.
const NEO_CLAUDE_INSTRUCTIONS: &str = r#"## NEO — Codebase Map (READ FIRST)

A `neo.md` file exists in the project root containing the full codebase map.

**ALWAYS before searching the codebase:**
1. Read `neo.md` first
2. Use `neo.md` to locate files directly
3. Only grep if `neo.md` doesn't have the answer

**ALWAYS after making file changes:**
Run: `neo update <space-separated list of files you changed>`
Example: `neo update src/auth/token.ts src/api/user.ts`

**NEVER:**
- Grep the codebase for something neo.md already maps
- Skip updating neo.md after changes
- Edit neo.md manually (it is auto-generated)

---
"#;

/// Writes Neo instructions to CLAUDE.md.
///
/// Behavior:
///   - If CLAUDE.md doesn't exist: creates it with Neo instructions
///   - If CLAUDE.md exists but has no Neo block: prepends instructions
///   - If CLAUDE.md already has Neo block: updates the block in place
///
/// This is idempotent — safe to run multiple times.
pub fn write_claude_instructions(project_root: &Path) -> Result<(), NeoError> {
    let claude_md_path = project_root.join("CLAUDE.md");

    if claude_md_path.exists() {
        let existing = std::fs::read_to_string(&claude_md_path)?;

        if existing.contains("## NEO — Codebase Map") {
            // Already has Neo block — update it in place
            let updated = replace_neo_block(&existing);
            std::fs::write(&claude_md_path, updated)?;
        } else {
            // Prepend Neo instructions to existing CLAUDE.md
            // Neo block goes FIRST so agent reads it immediately
            let updated = format!("{}\n{}", NEO_CLAUDE_INSTRUCTIONS, existing);
            std::fs::write(&claude_md_path, updated)?;
        }
    } else {
        // Create new CLAUDE.md with just Neo instructions
        std::fs::write(&claude_md_path, NEO_CLAUDE_INSTRUCTIONS)?;
    }

    Ok(())
}

/// Replaces an existing Neo block in CLAUDE.md with the current version.
///
/// Handles the case where Neo instructions have been updated
/// and need to be refreshed in an existing CLAUDE.md.
fn replace_neo_block(content: &str) -> String {
    // Find the start of the Neo block
    let start_marker = "## NEO — Codebase Map (READ FIRST)";

    if let Some(start_pos) = content.find(start_marker) {
        // Find the end of the Neo block (next ## header or end of file)
        let after_start = &content[start_pos..];
        let end_pos = after_start
            .find("\n## ")
            .map(|p| start_pos + p + 1) // +1 to keep the newline before ##
            .unwrap_or(content.len());

        // Replace the Neo block
        format!(
            "{}{}{}",
            &content[..start_pos],
            NEO_CLAUDE_INSTRUCTIONS,
            &content[end_pos..]
        )
    } else {
        // No existing block found — prepend
        format!("{}\n{}", NEO_CLAUDE_INSTRUCTIONS, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_creates_claude_md_if_missing() {
        let dir = tempdir().unwrap();
        write_claude_instructions(dir.path()).unwrap();

        let content = fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(content.contains("NEO — Codebase Map"));
        assert!(content.contains("neo update"));
    }

    #[test]
    fn test_prepends_to_existing_claude_md() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("CLAUDE.md");
        fs::write(&path, "# My Project\nSome existing instructions.").unwrap();

        write_claude_instructions(dir.path()).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        // Neo block should come before existing content
        let neo_pos = content.find("NEO").unwrap();
        let existing_pos = content.find("My Project").unwrap();
        assert!(neo_pos < existing_pos);
    }

    #[test]
    fn test_idempotent_on_existing_neo_block() {
        let dir = tempdir().unwrap();
        write_claude_instructions(dir.path()).unwrap();
        write_claude_instructions(dir.path()).unwrap(); // Run twice

        let content = fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        // Should only have one Neo block
        let count = content.matches("NEO — Codebase Map").count();
        assert_eq!(count, 1);
    }
}
