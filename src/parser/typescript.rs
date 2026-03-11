/// parser/typescript.rs
///
/// TypeScript and TSX parser.
/// Also handles plain JavaScript (.js, .jsx) since syntax is compatible.
///
/// Extracts:
///   - Import statements (ES modules and dynamic imports)
///   - Export statements (named and default exports)
///
/// Implementation: regex-based for now.
/// Future: swap for a proper AST parser (swc or oxc bindings)
/// without changing the Parser trait interface.

use std::path::Path;
use regex::Regex;
use crate::types::NeoError;
use super::Parser;

pub struct TypeScriptParser {
    /// Matches: import x from './path'
    ///          import { x } from '../path'
    ///          import type { x } from './path'
    import_regex: Regex,

    /// Matches: export function x
    ///          export const x
    ///          export class x
    ///          export default x
    ///          export type x
    export_regex: Regex,
}

impl TypeScriptParser {
    pub fn new() -> Self {
        TypeScriptParser {
            // Captures the path from various import statement forms
            // Group 1: the import path string
            import_regex: Regex::new(
                r#"(?:import|from)\s+['"]([^'"]+)['"]"#
            ).unwrap(),

            // Captures the name of exported symbols
            // Group 1: the exported name
            export_regex: Regex::new(
                r#"export\s+(?:default\s+)?(?:function|class|const|let|var|type|interface|enum)\s+(\w+)"#
            ).unwrap(),
        }
    }

    /// Resolves a raw import path to a normalized project-relative path.
    ///
    /// Raw import:    "../auth/token"
    /// Normalized:    "src/auth/token.ts"
    ///
    /// Rules:
    ///   - Only resolve relative imports (starting with ./ or ../)
    ///   - Skip node_modules imports (no leading dot)
    ///   - Try .ts extension first, then .tsx, then index files
    fn resolve_import(
        &self,
        raw_import: &str,
        importing_file: &Path,
        project_root: &Path,
    ) -> Option<String> {
        // Skip node_modules imports — not part of the codebase
        // e.g. "react", "lodash", "@types/node"
        if !raw_import.starts_with('.') {
            return None;
        }

        // Resolve the import relative to the importing file's directory
        let base_dir = importing_file.parent()?;
        let resolved = base_dir.join(raw_import);

        // Try common TypeScript file extensions in priority order
        let extensions = ["ts", "tsx", "js", "jsx"];
        for ext in &extensions {
            let with_ext = resolved.with_extension(ext);
            if with_ext.exists() {
                // Convert back to project-relative path
                if let Ok(relative) = with_ext.strip_prefix(project_root) {
                    return Some(relative.to_string_lossy().to_string());
                }
            }
        }

        // Try as directory with index file
        // e.g. import from '../auth' → src/auth/index.ts
        let extensions_index = ["ts", "tsx", "js"];
        for ext in &extensions_index {
            let index_file = resolved.join(format!("index.{}", ext));
            if index_file.exists() {
                if let Ok(relative) = index_file.strip_prefix(project_root) {
                    return Some(relative.to_string_lossy().to_string());
                }
            }
        }

        None
    }
}

impl Parser for TypeScriptParser {
    fn can_parse(&self, filepath: &Path) -> bool {
        match filepath.extension().and_then(|e| e.to_str()) {
            Some(ext) => matches!(ext, "ts" | "tsx" | "js" | "jsx"),
            None => false,
        }
    }

    fn extract_deps(&self, filepath: &Path, project_root: &Path) -> Result<Vec<String>, NeoError> {
        // Read file contents
        let content = std::fs::read_to_string(filepath).map_err(|e| NeoError::ParseError {
            path: filepath.to_string_lossy().to_string(),
            reason: e.to_string(),
        })?;

        let mut deps = Vec::new();

        // Find all import statements and resolve them
        for capture in self.import_regex.captures_iter(&content) {
            let raw_import = &capture[1];

            if let Some(resolved) = self.resolve_import(raw_import, filepath, project_root) {
                // Avoid duplicate deps (same file imported multiple times)
                if !deps.contains(&resolved) {
                    deps.push(resolved);
                }
            }
        }

        Ok(deps)
    }

    fn extract_exports(&self, filepath: &Path) -> Vec<String> {
        // Read file — if it fails, return empty (exports are optional)
        let content = match std::fs::read_to_string(filepath) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut exports = Vec::new();

        for capture in self.export_regex.captures_iter(&content) {
            exports.push(capture[1].to_string());
        }

        exports
    }

    fn extensions(&self) -> Vec<&'static str> {
        vec!["ts", "tsx", "js", "jsx"]
    }

    fn language_name(&self) -> &'static str {
        "TypeScript"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_can_parse_ts_files() {
        let parser = TypeScriptParser::new();
        assert!(parser.can_parse(Path::new("src/auth/token.ts")));
        assert!(parser.can_parse(Path::new("src/App.tsx")));
        assert!(!parser.can_parse(Path::new("main.py")));
        assert!(!parser.can_parse(Path::new("README.md")));
    }

    #[test]
    fn test_extract_exports() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("auth.ts");

        fs::write(&file, r#"
            export function createToken(user: User): string { }
            export const TOKEN_EXPIRY = 3600;
            export class TokenService { }
        "#).unwrap();

        let parser = TypeScriptParser::new();
        let exports = parser.extract_exports(&file);

        assert!(exports.contains(&"createToken".to_string()));
        assert!(exports.contains(&"TOKEN_EXPIRY".to_string()));
        assert!(exports.contains(&"TokenService".to_string()));
    }
}
