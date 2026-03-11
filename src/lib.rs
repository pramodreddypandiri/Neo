/// lib.rs
///
/// Exposes Neo's core engine as a reusable library.
///
/// This allows other tools to import neo_core and use its
/// parsing, graph building, and writing capabilities
/// without going through the CLI.

pub mod types;
pub mod core;
pub mod parser;
pub mod ai;
pub mod agent;

// Re-export the most commonly used types at the top level
// for convenience when using neo as a library
pub use types::{Neo, NeoFile, NeoConvention, NeoConfig, EntryPoint, NeoError};