/// types.rs
/// 
/// Core data structures shared across all Neo modules.
/// Every module imports from here — nothing else.
/// 
/// Design principle: keep types minimal and flat.
/// Nested structures balloon neo.md size unnecessarily.

use serde::{Deserialize, Serialize};

/// Represents a single file entry in Neo.
/// 
/// This is the fundamental unit Neo tracks.
/// One NeoFile = one file in the codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeoFile {
    /// Relative path from project root
    /// e.g. "src/auth/token.ts"
    pub path: String,

    /// One-line AI-generated description of what this file does
    /// e.g. "JWT creation, validation and refresh logic"
    /// Kept to one line intentionally — every extra word costs tokens
    pub purpose: String,

    /// Files this file directly imports/requires/depends on
    /// e.g. ["src/config/env.ts", "src/lib/errors.ts"]
    /// Only direct deps, not transitive — keeps Neo lean
    pub deps: Vec<String>,

    /// Files that import THIS file (reverse dependency)
    /// Computed by the graph builder, not the parser
    /// High count = hotspot candidate
    pub dependents: Vec<String>,

    /// Number of files that depend on this file
    /// Cached here so hotspot detection is O(1)
    pub hotspot_score: u32,
}

/// Metadata about the Neo index itself
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeoConfig {
    /// Detected language of the codebase
    /// e.g. "typescript", "python", "rust"
    pub language: String,

    /// ISO 8601 timestamp when Neo was last generated/updated
    pub generated_at: String,

    /// Total number of files tracked in this Neo index
    pub file_count: u32,

    /// Neo format version — increment when format changes
    /// Allows future tooling to handle old neo.md files gracefully
    pub version: u8,
}

/// A detected or manually specified coding convention
/// 
/// Examples:
///   key: "error-handling"  value: "AppError class"    confidence: 0.91
///   key: "api-calls"       value: "services/api/* only" confidence: 0.87
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeoConvention {
    /// Short label for the convention
    /// e.g. "error-handling", "state-management", "api-calls"
    pub key: String,

    /// Human-readable description of the convention
    /// e.g. "Always use AppError class from utils/errors.ts"
    pub value: String,

    /// 0.0 to 1.0 — how consistently this pattern appears in codebase
    /// 0.9+ = very consistent, 0.5 = mixed, below 0.5 = not really a convention
    pub confidence: f32,

    /// Whether the developer has explicitly confirmed this convention
    /// Unconfirmed conventions are shown to user during `neo init`
    /// Only confirmed ones are written to neo.md
    pub confirmed: bool,
}

/// A named entry point into the codebase
/// 
/// Examples:
///   role: "app-entry"   path: "index.ts"
///   role: "navigation"  path: "src/navigation/RootNavigator.tsx"
///   role: "api-base"    path: "src/services/api/httpClient.ts"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    /// What role this entry point serves
    /// e.g. "app-entry", "navigation", "db-config", "env-config"
    pub role: String,

    /// Relative path to the entry point file
    pub path: String,
}

/// The complete Neo index for a codebase
/// 
/// This is what gets serialized to neo.md.
/// All other types are components of this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neo {
    /// Index metadata (language, timestamp, version)
    pub config: NeoConfig,

    /// All tracked files with their purposes and dependencies
    pub files: Vec<NeoFile>,

    /// Detected coding conventions for this codebase
    pub conventions: Vec<NeoConvention>,

    /// Key entry points the coding agent should know about
    pub entry_points: Vec<EntryPoint>,
}

impl Neo {
    /// Creates a new empty Neo index
    pub fn new(language: String) -> Self {
        Neo {
            config: NeoConfig {
                language,
                generated_at: chrono::Utc::now().to_rfc3339(),
                file_count: 0,
                version: 1,
            },
            files: Vec::new(),
            conventions: Vec::new(),
            entry_points: Vec::new(),
        }
    }

    /// Returns files sorted by hotspot score descending
    /// Used when writing the HOTSPOTS section of neo.md
    pub fn hotspots(&self) -> Vec<&NeoFile> {
        let mut sorted: Vec<&NeoFile> = self.files.iter()
            .filter(|f| f.hotspot_score > 5) // only meaningful hotspots
            .collect();
        sorted.sort_by(|a, b| b.hotspot_score.cmp(&a.hotspot_score));
        sorted
    }

    /// Finds a file entry by its path
    #[allow(dead_code)]
    pub fn find_file(&self, path: &str) -> Option<&NeoFile> {
        self.files.iter().find(|f| f.path == path)
    }

    /// Updates or inserts a file entry
    /// Used by `neo update` for incremental updates
    pub fn upsert_file(&mut self, file: NeoFile) {
        if let Some(existing) = self.files.iter_mut().find(|f| f.path == file.path) {
            *existing = file;
        } else {
            self.files.push(file);
        }
        self.config.file_count = self.files.len() as u32;
        self.config.generated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Removes a file entry by path
    /// Used when a file is deleted from the codebase
    pub fn remove_file(&mut self, path: &str) {
        self.files.retain(|f| f.path != path);
        self.config.file_count = self.files.len() as u32;
    }
}

/// Errors that can occur in the Neo core engine
/// 
/// Using thiserror for typed errors in the library layer.
/// CLI layer converts these to anyhow errors for display.
#[derive(Debug, thiserror::Error)]
pub enum NeoError {
    #[error("neo.md not found. Run `neo init` first.")]
    NotInitialized,

    #[error("Failed to parse file: {path} — {reason}")]
    ParseError { path: String, reason: String },

    #[error("AI summarization failed: {0}")]
    AiError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
}
