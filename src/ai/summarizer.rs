/// ai/summarizer.rs
///
/// Generates one-line purpose summaries for source files.
///
/// This is the ONLY place Neo makes AI API calls during normal operation.
/// Everything else in Neo is static analysis.
///
/// Strategy:
///   - Batch multiple files per API call to reduce overhead tokens
///   - Use a tightly constrained prompt to force one-line output
///   - Cache results so re-running neo init doesn't re-summarize unchanged files
///
/// AI provider: Anthropic Claude (default), swappable via config

use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::types::NeoError;

/// How many files to summarize in a single API call.
/// Batching reduces the per-file prompt overhead significantly.
/// 10 is a sweet spot — large enough to save tokens, small enough
/// that one bad file doesn't corrupt the whole batch response.
const BATCH_SIZE: usize = 10;

/// A file to be summarized, containing its path and content preview.
pub struct FileToSummarize {
    pub path: String,
    pub content_preview: String, // First ~500 chars — enough for purpose
    pub exports: Vec<String>,    // Known exports help AI understand the file
}

/// Summarizes a batch of files in a single AI call.
///
/// Returns a map of filepath → one-line summary.
/// Files that fail to summarize get a fallback description.
pub async fn summarize_files(
    files: Vec<FileToSummarize>,
    api_key: &str,
) -> Result<Vec<(String, String)>, NeoError> {
    let mut results = Vec::new();

    // Process files in batches
    for chunk in files.chunks(BATCH_SIZE) {
        let batch_results = summarize_batch(chunk, api_key).await?;
        results.extend(batch_results);
    }

    Ok(results)
}

/// Summarizes a single batch of files in one API call.
async fn summarize_batch(
    files: &[FileToSummarize],
    api_key: &str,
) -> Result<Vec<(String, String)>, NeoError> {
    // Build a compact prompt listing all files in the batch
    // Tight prompt = fewer input tokens
    let mut prompt = String::from(
        "For each file below, write ONE line (max 10 words) describing what it does.\n\
         Format: exactly one line per file, in the same order.\n\
         Be specific — mention the main function/class if obvious.\n\
         Do not number lines. Do not add explanations.\n\n"
    );

    for file in files {
        // Include exports as hints for the AI
        let exports_hint = if !file.exports.is_empty() {
            format!(" [exports: {}]", file.exports.join(", "))
        } else {
            String::new()
        };

        prompt.push_str(&format!(
            "FILE: {}{}\n{}\n---\n",
            file.path,
            exports_hint,
            // Truncate content to keep prompt lean
            &file.content_preview[..file.content_preview.len().min(400)],
        ));
    }

    // Call Anthropic API
    let response = call_anthropic(&prompt, api_key).await?;

    // Parse response — one line per file, in order
    let lines: Vec<&str> = response
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    let mut results = Vec::new();
    for (i, file) in files.iter().enumerate() {
        let summary = lines
            .get(i)
            .map(|l| l.trim().to_string())
            // Fallback if AI response has fewer lines than expected
            .unwrap_or_else(|| fallback_summary(&file.path));

        results.push((file.path.clone(), summary));
    }

    Ok(results)
}

/// Calls the Anthropic Messages API with a prompt.
///
/// Returns the text content of the first response block.
async fn call_anthropic(prompt: &str, api_key: &str) -> Result<String, NeoError> {
    #[derive(Serialize)]
    struct Request {
        model: String,
        max_tokens: u32,
        messages: Vec<Message>,
    }

    #[derive(Serialize)]
    struct Message {
        role: String,
        content: String,
    }

    #[derive(Deserialize)]
    struct Response {
        content: Vec<ContentBlock>,
    }

    #[derive(Deserialize)]
    struct ContentBlock {
        #[serde(rename = "type")]
        block_type: String,
        text: Option<String>,
    }

    let client = reqwest::Client::new();

    let request = Request {
        model: "claude-haiku-4-5-20251001".to_string(), // Haiku: fastest, cheapest for summarization
        max_tokens: 500, // One line per file × 10 files × ~10 tokens = well under 500
        messages: vec![Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| NeoError::AiError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(NeoError::AiError(format!(
            "API returned {}: {}",
            status, body
        )));
    }

    let parsed: Response = response
        .json()
        .await
        .map_err(|e| NeoError::AiError(e.to_string()))?;

    // Extract text from first content block
    let text = parsed
        .content
        .into_iter()
        .find(|b| b.block_type == "text")
        .and_then(|b| b.text)
        .unwrap_or_default();

    Ok(text)
}

/// Generates a fallback summary when AI summarization fails.
///
/// Uses the filename itself as a hint — better than nothing.
/// e.g. "src/auth/token.ts" → "token.ts — purpose unknown"
fn fallback_summary(path: &str) -> String {
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    format!("{} — purpose unknown", filename)
}

/// Reads the first N characters of a file for use as a content preview.
///
/// Truncating is fine — we only need enough to understand the file's purpose.
/// Full file content would balloon the batch prompt unnecessarily.
pub fn read_content_preview(filepath: &Path, max_chars: usize) -> String {
    std::fs::read_to_string(filepath)
        .unwrap_or_default()
        .chars()
        .take(max_chars)
        .collect()
}
