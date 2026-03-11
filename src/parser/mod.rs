/// parser/mod.rs
///
/// Defines the Parser trait — the core abstraction that makes Neo
/// language-agnostic. Every supported language implements this trait.
///
/// Design principle: parsers do ONLY static analysis.
/// They extract structure (imports, exports) from files.
/// They do NOT generate summaries — that is the AI layer's job.
///
/// Adding a new language = implementing this trait. Nothing else changes.

use std::path::Path;
use crate::types::NeoError;

pub mod detector;
pub mod typescript;
pub mod python;

/// The Parser trait — implement this for every supported language.
///
/// Parsers are intentionally minimal. They answer two questions:
///   1. What does this file import? (deps)
///   2. What does this file export? (exports)
///
/// Everything else (purpose, hotspot score) is derived by the core engine.
pub trait Parser: Send + Sync {
    /// Returns true if this parser can handle the given file.
    ///
    /// Typically checks file extension.
    /// e.g. TypeScript parser returns true for .ts and .tsx files
    fn can_parse(&self, filepath: &Path) -> bool;

    /// Extracts direct dependencies from a file.
    ///
    /// Returns a list of resolved relative paths that this file imports.
    /// e.g. for TypeScript: parses `import x from '../auth/token'`
    ///      returns: ["src/auth/token.ts"]
    ///
    /// Important: return normalized relative paths, not raw import strings.
    /// Raw: "../auth/token"  →  Normalized: "src/auth/token.ts"
    fn extract_deps(&self, filepath: &Path, project_root: &Path) -> Result<Vec<String>, NeoError>;

    /// Extracts what this file exports (functions, classes, types, constants).
    ///
    /// Used to build a high-level understanding of the file's public API.
    /// e.g. ["createToken", "validateToken", "refreshToken"]
    ///
    /// Kept optional — returns empty vec if extraction fails.
    /// We never want a parser failure to block Neo generation.
    fn extract_exports(&self, filepath: &Path) -> Vec<String>;

    /// Returns file extensions this parser handles.
    ///
    /// Used by the detector to map extensions to parsers.
    /// e.g. TypeScript parser returns ["ts", "tsx"]
    fn extensions(&self) -> Vec<&'static str>;

    /// Human-readable name of this parser.
    /// e.g. "TypeScript", "Python", "Go"
    fn language_name(&self) -> &'static str;
}

/// Returns a parser for the given file, if one exists.
///
/// Tries all registered parsers in order.
/// Returns None if no parser handles this file type.
pub fn get_parser_for_file(filepath: &Path) -> Option<Box<dyn Parser>> {
    let parsers: Vec<Box<dyn Parser>> = vec![
        Box::new(typescript::TypeScriptParser::new()),
        Box::new(python::PythonParser::new()),
    ];

    parsers.into_iter().find(|p| p.can_parse(filepath))
}
