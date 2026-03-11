/// core/graph.rs
///
/// Builds a dependency graph from parsed file data.
///
/// The graph answers:
///   - "What files does X depend on?"  (deps)
///   - "What files depend on X?"       (dependents / reverse deps)
///
/// The reverse dependency ("who imports me?") is what enables
/// hotspot detection — files with many dependents are hotspots.
///
/// This runs after all files have been parsed.
/// Input:  list of (filepath, deps) pairs
/// Output: enriched NeoFile list with dependents populated

use std::collections::HashMap;
use crate::types::NeoFile;

/// Builds reverse dependencies for all files.
///
/// Given a list of NeoFiles where each has `deps` populated,
/// this function fills in the `dependents` and `hotspot_score` fields.
///
/// Algorithm:
///   For each file F:
///     For each dep D of F:
///       Add F to D's dependents list
///
/// O(n * avg_deps) — linear in practice
pub fn build_reverse_deps(files: &mut Vec<NeoFile>) {
    // Build a map of path → index for O(1) lookups
    // We need this to update dependents without borrowing issues
    let path_to_index: HashMap<String, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.clone(), i))
        .collect();

    // Collect all (dependent, dependency) pairs first
    // Can't mutate files while iterating over it
    let mut reverse_edges: Vec<(usize, String)> = Vec::new();

    for (idx, file) in files.iter().enumerate() {
        for dep in &file.deps {
            // Only add reverse edge if the dep is tracked in Neo
            // (external deps like node_modules are not tracked)
            if path_to_index.contains_key(dep) {
                reverse_edges.push((idx, dep.clone()));
            }
        }
    }

    // Now apply reverse edges
    for (dependent_idx, dep_path) in reverse_edges {
        if let Some(&dep_idx) = path_to_index.get(&dep_path) {
            let dependent_path = files[dependent_idx].path.clone();
            let dep_file = &mut files[dep_idx];

            // Avoid adding the same dependent twice
            if !dep_file.dependents.contains(&dependent_path) {
                dep_file.dependents.push(dependent_path);
            }
        }
    }

    // Update hotspot scores — just the count of dependents
    for file in files.iter_mut() {
        file.hotspot_score = file.dependents.len() as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(path: &str, deps: Vec<&str>) -> NeoFile {
        NeoFile {
            path: path.to_string(),
            purpose: String::new(),
            deps: deps.into_iter().map(String::from).collect(),
            dependents: Vec::new(),
            hotspot_score: 0,
        }
    }

    #[test]
    fn test_builds_reverse_deps() {
        let mut files = vec![
            make_file("src/app.ts", vec!["src/utils/http.ts"]),
            make_file("src/auth.ts", vec!["src/utils/http.ts"]),
            make_file("src/utils/http.ts", vec![]),
        ];

        build_reverse_deps(&mut files);

        // http.ts should have both app.ts and auth.ts as dependents
        let http = files.iter().find(|f| f.path == "src/utils/http.ts").unwrap();
        assert_eq!(http.hotspot_score, 2);
        assert!(http.dependents.contains(&"src/app.ts".to_string()));
        assert!(http.dependents.contains(&"src/auth.ts".to_string()));
    }

    #[test]
    fn test_hotspot_score() {
        let mut files = vec![
            make_file("a.ts", vec!["shared.ts"]),
            make_file("b.ts", vec!["shared.ts"]),
            make_file("c.ts", vec!["shared.ts"]),
            make_file("shared.ts", vec![]),
        ];

        build_reverse_deps(&mut files);

        let shared = files.iter().find(|f| f.path == "shared.ts").unwrap();
        assert_eq!(shared.hotspot_score, 3);
    }
}
