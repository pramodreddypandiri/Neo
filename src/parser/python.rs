/// parser/python.rs
///
/// Python parser.
/// Handles .py files.
///
/// Extracts:
///   - import statements (import x, from x import y)
///   - top-level def and class exports

use std::path::Path;
use regex::Regex;
use crate::types::NeoError;
use super::Parser;

pub struct PythonParser {
    /// Matches: import os.path
    ///          from .auth import token
    ///          from ..utils import helpers
    import_regex: Regex,

    /// Matches top-level functions and classes
    /// e.g. def create_token, class UserModel
    export_regex: Regex,
}

impl PythonParser {
    pub fn new() -> Self {
        PythonParser {
            import_regex: Regex::new(
                r#"(?:^from\s+([\.\w]+)\s+import|^import\s+([\.\w]+))"#
            ).unwrap(),
            export_regex: Regex::new(
                r#"^(?:def|class)\s+([A-Za-z_]\w*)"#
            ).unwrap(),
        }
    }

    /// Resolves a Python import to a project-relative file path.
    ///
    /// Python imports use dot notation, not file paths:
    ///   "from .auth import token"  →  resolve relative to current package
    ///   "from utils import helpers" →  resolve from project root
    fn resolve_import(
        &self,
        raw_import: &str,
        importing_file: &Path,
        project_root: &Path,
    ) -> Option<String> {
        // Skip standard library and third-party imports
        // Relative imports start with a dot in Python
        // Absolute imports without dot are either stdlib or third-party
        // We use a heuristic: if the file exists in project, it's local
        let base = if raw_import.starts_with('.') {
            // Relative import — resolve from current file's package
            let dots = raw_import.chars().take_while(|c| *c == '.').count();
            let mut base = importing_file.parent()?;
            // Each dot = one level up
            for _ in 1..dots {
                base = base.parent()?;
            }
            base.to_path_buf()
        } else {
            // Absolute import — try from project root
            project_root.to_path_buf()
        };

        // Convert dot notation to path
        // e.g. "auth.token" → "auth/token"
        let clean = raw_import.trim_start_matches('.');
        let as_path = clean.replace('.', "/");
        let candidate = base.join(format!("{}.py", as_path));

        if candidate.exists() {
            if let Ok(relative) = candidate.strip_prefix(project_root) {
                return Some(relative.to_string_lossy().to_string());
            }
        }

        // Try as package (directory with __init__.py)
        let init = base.join(&as_path).join("__init__.py");
        if init.exists() {
            if let Ok(relative) = init.strip_prefix(project_root) {
                return Some(relative.to_string_lossy().to_string());
            }
        }

        None
    }
}

impl Parser for PythonParser {
    fn can_parse(&self, filepath: &Path) -> bool {
        filepath.extension().and_then(|e| e.to_str()) == Some("py")
    }

    fn extract_deps(&self, filepath: &Path, project_root: &Path) -> Result<Vec<String>, NeoError> {
        let content = std::fs::read_to_string(filepath).map_err(|e| NeoError::ParseError {
            path: filepath.to_string_lossy().to_string(),
            reason: e.to_string(),
        })?;

        let mut deps = Vec::new();

        for capture in self.import_regex.captures_iter(&content) {
            // Group 1: from X import  |  Group 2: import X
            let raw = capture.get(1).or(capture.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");

            if let Some(resolved) = self.resolve_import(raw, filepath, project_root) {
                if !deps.contains(&resolved) {
                    deps.push(resolved);
                }
            }
        }

        Ok(deps)
    }

    fn extract_exports(&self, filepath: &Path) -> Vec<String> {
        let content = match std::fs::read_to_string(filepath) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        self.export_regex
            .captures_iter(&content)
            .map(|c| c[1].to_string())
            .collect()
    }

    fn extensions(&self) -> Vec<&'static str> {
        vec!["py"]
    }

    fn language_name(&self) -> &'static str {
        "Python"
    }
}
