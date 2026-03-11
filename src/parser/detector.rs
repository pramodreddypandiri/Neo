/// parser/detector.rs
///
/// Auto-detects the primary language of a codebase.
///
/// Strategy: look for well-known marker files in the project root.
/// Each language ecosystem has a standard config/manifest file.
/// First match wins.
///
/// This runs once during `neo init`.
/// Result is stored in NeoConfig.language.

use std::path::Path;

/// Detects the primary language of the codebase at the given root.
///
/// Returns a lowercase language string e.g. "typescript", "python", "rust"
/// Returns None if language cannot be determined.
///
/// Marker files checked (in priority order):
///   package.json + tsconfig.json → typescript
///   package.json                 → javascript
///   requirements.txt / pyproject.toml / setup.py → python
///   Cargo.toml                   → rust
///   go.mod                       → go
///   pom.xml / build.gradle       → java
///   Gemfile                      → ruby
///   mix.exs                      → elixir
pub fn detect_language(project_root: &Path) -> Option<String> {
    // TypeScript: has both package.json AND tsconfig.json
    // Check this before JavaScript to avoid misclassifying TS projects
    if project_root.join("tsconfig.json").exists()
        && project_root.join("package.json").exists()
    {
        return Some("typescript".to_string());
    }

    // JavaScript: has package.json but no tsconfig
    if project_root.join("package.json").exists() {
        return Some("javascript".to_string());
    }

    // Python: any of the common Python project markers
    if project_root.join("requirements.txt").exists()
        || project_root.join("pyproject.toml").exists()
        || project_root.join("setup.py").exists()
    {
        return Some("python".to_string());
    }

    // Rust: Cargo.toml is always present in Rust projects
    if project_root.join("Cargo.toml").exists() {
        return Some("rust".to_string());
    }

    // Go: go.mod is the standard Go module file
    if project_root.join("go.mod").exists() {
        return Some("go".to_string());
    }

    // Java: Maven (pom.xml) or Gradle (build.gradle)
    if project_root.join("pom.xml").exists()
        || project_root.join("build.gradle").exists()
    {
        return Some("java".to_string());
    }

    // Ruby: Gemfile is standard for Ruby projects
    if project_root.join("Gemfile").exists() {
        return Some("ruby".to_string());
    }

    // Elixir: mix.exs is the Elixir build tool config
    if project_root.join("mix.exs").exists() {
        return Some("elixir".to_string());
    }

    // Could not determine language
    // neo init will ask the user to specify manually
    None
}

/// Returns true if Neo has a parser implemented for this language.
///
/// Used to warn users when a language is detected but not yet supported.
pub fn is_supported(language: &str) -> bool {
    matches!(language, "typescript" | "javascript" | "python")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_detects_typescript() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        assert_eq!(detect_language(dir.path()), Some("typescript".to_string()));
    }

    #[test]
    fn test_detects_python() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"), "").unwrap();
        assert_eq!(detect_language(dir.path()), Some("python".to_string()));
    }

    #[test]
    fn test_detects_rust() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_language(dir.path()), Some("rust".to_string()));
    }

    #[test]
    fn test_unknown_language() {
        let dir = tempdir().unwrap();
        assert_eq!(detect_language(dir.path()), None);
    }
}
