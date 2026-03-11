/// core/scanner.rs
///
/// Walks the project directory tree and collects files to be indexed.
///
/// Responsibilities:
///   - Traverse directory recursively
///   - Skip files/dirs that should never be in Neo
///     (node_modules, .git, build outputs, assets)
///   - Return only source files that parsers can handle
///
/// Design: scanner is intentionally dumb — it only finds files.
/// It does not parse them. Parsing is the parser layer's job.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Directories that should always be excluded from scanning.
///
/// These never contain meaningful source code for Neo purposes:
///   - Generated/vendor code (node_modules, .venv, vendor)
///   - Version control (.git, .svn)
///   - Build outputs (dist, build, target, __pycache__)
///   - IDE config (.idea, .vscode)
///   - Neo's own output (prevents neo from indexing itself)
const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    "dist",
    "build",
    "target",         // Rust build output
    "__pycache__",    // Python bytecode cache
    ".venv",          // Python virtual environment
    "venv",           // Python virtual environment (alt name)
    ".env",           // Environment directory (not .env file)
    "vendor",         // Go and PHP vendor directories
    ".idea",          // JetBrains IDE config
    ".vscode",        // VS Code config
    "coverage",       // Test coverage reports
    ".next",          // Next.js build output
    ".expo",          // Expo build output (React Native)
    "android/build",  // React Native Android build
    "ios/build",      // React Native iOS build
    "assets",         // Static assets (images, fonts) — no source code
];

/// File extensions that are almost never useful in Neo.
/// Even if a parser could handle them, we skip them.
const EXCLUDED_EXTENSIONS: &[&str] = &[
    // Lock files — generated, not authored
    "lock",
    // Image files
    "png", "jpg", "jpeg", "gif", "svg", "ico", "webp",
    // Font files
    "ttf", "woff", "woff2", "eot",
    // Binary/compiled
    "exe", "dll", "so", "dylib", "bin",
    // Archives
    "zip", "tar", "gz",
    // Documentation (keep README though — handled separately)
    "pdf",
    // Environment files (contain secrets, not structure)
    "env",
];

/// Result of a directory scan
pub struct ScanResult {
    /// All source files found, as absolute paths
    pub files: Vec<PathBuf>,

    /// Number of files skipped (for progress reporting)
    pub skipped: u32,
}

/// Scans a project directory and returns all indexable source files.
///
/// `project_root` — absolute path to the project root
///
/// Returns a ScanResult with all files that parsers should process.
pub fn scan_project(project_root: &Path) -> ScanResult {
    let mut files = Vec::new();
    let mut skipped = 0u32;

    for entry in WalkDir::new(project_root)
        .follow_links(false) // don't follow symlinks — avoids infinite loops
        .into_iter()
        .filter_entry(|e| !should_exclude_dir(e.path()))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        // Skip directories themselves — we only want files
        if entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();

        // Skip files with excluded extensions
        if should_exclude_file(path) {
            skipped += 1;
            continue;
        }

        files.push(path.to_path_buf());
    }

    ScanResult { files, skipped }
}

/// Returns true if this directory should be completely skipped.
///
/// Called by WalkDir's filter_entry — when we return false here,
/// WalkDir won't descend into that directory at all (efficient).
fn should_exclude_dir(path: &Path) -> bool {
    // Check if any path component matches excluded dir names
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        EXCLUDED_DIRS.contains(&name.as_ref())
    })
}

/// Returns true if this file should be skipped.
fn should_exclude_file(path: &Path) -> bool {
    // Skip files with excluded extensions
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if EXCLUDED_EXTENSIONS.contains(&ext) {
            return true;
        }
    }

    // Skip hidden files (dotfiles) at root level
    // e.g. .eslintrc, .prettierrc — not source files
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.starts_with('.') {
            return true;
        }
    }

    // Skip test files — valuable but add noise to Neo
    // Future: make this configurable
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.contains(".test.") || name.contains(".spec.") {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_skips_node_modules() {
        let dir = tempdir().unwrap();
        let nm = dir.path().join("node_modules/lodash/index.js");
        fs::create_dir_all(nm.parent().unwrap()).unwrap();
        fs::write(&nm, "").unwrap();

        let src = dir.path().join("src/index.ts");
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        fs::write(&src, "").unwrap();

        let result = scan_project(dir.path());

        // node_modules file should be skipped
        assert!(!result.files.contains(&nm));
        // src file should be found
        assert!(result.files.contains(&src));
    }

    #[test]
    fn test_skips_images() {
        let dir = tempdir().unwrap();
        let img = dir.path().join("logo.png");
        fs::write(&img, "").unwrap();

        let ts = dir.path().join("app.ts");
        fs::write(&ts, "").unwrap();

        let result = scan_project(dir.path());

        assert!(!result.files.contains(&img));
        assert!(result.files.contains(&ts));
    }
}
