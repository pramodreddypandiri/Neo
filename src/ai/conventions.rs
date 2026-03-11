/// ai/conventions.rs
///
/// Infers coding conventions from the codebase.
///
/// Strategy:
///   - Sample a representative set of files (not all — too expensive)
///   - Ask AI to identify recurring patterns
///   - Return candidates with confidence scores
///   - User confirms before writing to neo.md
///
/// This runs ONCE during `neo init`.
/// Result is written to neo.md after user confirmation.

use crate::types::{NeoConvention, NeoError};

/// Number of files to sample for convention detection.
/// 20 files is enough to detect patterns without over-spending tokens.
const SAMPLE_SIZE: usize = 20;

/// Infers conventions from a sample of source files.
///
/// `file_contents` — vec of (filepath, content) pairs
///
/// Returns a list of detected conventions with confidence scores.
/// These are NOT confirmed yet — user must confirm during `neo init`.
pub async fn infer_conventions(
    file_contents: Vec<(String, String)>,
    api_key: &str,
) -> Result<Vec<NeoConvention>, NeoError> {
    if file_contents.is_empty() {
        return Ok(Vec::new());
    }

    // Sample files — prefer files from different directories
    // for better coverage of the codebase's patterns
    let sample = sample_files(file_contents, SAMPLE_SIZE);

    // Build prompt with sampled file contents
    let prompt = build_conventions_prompt(&sample);

    // Call AI
    let response = call_anthropic_for_conventions(&prompt, api_key).await?;

    // Parse the structured response into NeoConvention objects
    parse_conventions_response(&response)
}

/// Samples files from different directories for representative coverage.
///
/// Simple strategy: take up to N files, preferring variety.
/// Sort by path so we get files from different directories.
fn sample_files(
    mut files: Vec<(String, String)>,
    max: usize,
) -> Vec<(String, String)> {
    // Sort by path to group similar files, then take evenly-spaced samples
    files.sort_by(|a, b| a.0.cmp(&b.0));

    if files.len() <= max {
        return files;
    }

    // Take every N-th file to spread across the codebase
    let step = files.len() / max;
    files
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % step == 0)
        .take(max)
        .map(|(_, f)| f)
        .collect()
}

/// Builds the prompt for convention detection.
///
/// Asks AI for specific, actionable conventions — not vague observations.
fn build_conventions_prompt(files: &[(String, String)]) -> String {
    let mut prompt = String::from(
        "Analyze these source files and identify recurring coding conventions.\n\
         \n\
         For each convention found, output EXACTLY this format:\n\
         KEY: value [confidence: 0.0-1.0]\n\
         \n\
         Keys to look for (only include if clearly present):\n\
         - error-handling: how errors are thrown/handled\n\
         - state-management: how state is managed (Redux, Zustand, etc)\n\
         - api-calls: where/how API calls are made\n\
         - styling: how UI is styled\n\
         - env-vars: how environment variables are accessed\n\
         - imports: any import conventions or aliases\n\
         - testing: testing framework and patterns\n\
         - types: how TypeScript types are organized\n\
         \n\
         Only include conventions with confidence >= 0.7.\n\
         Do not include conventions you are not sure about.\n\
         \n\
         FILES:\n"
    );

    for (path, content) in files {
        prompt.push_str(&format!(
            "\n--- {} ---\n{}\n",
            path,
            // First 300 chars per file keeps prompt manageable
            &content[..content.len().min(300)],
        ));
    }

    prompt
}

/// Parses the AI response into NeoConvention structs.
///
/// Expected format: "KEY: value [confidence: 0.85]"
fn parse_conventions_response(response: &str) -> Result<Vec<NeoConvention>, NeoError> {
    let mut conventions = Vec::new();

    for line in response.lines() {
        let line = line.trim();

        // Skip empty lines and lines that don't look like conventions
        if line.is_empty() || !line.contains(':') {
            continue;
        }

        // Parse "KEY: value [confidence: 0.85]"
        let (key_value, confidence) = if let Some(conf_start) = line.rfind('[') {
            let conf_str = line[conf_start..].trim_matches(|c| c == '[' || c == ']');
            let conf_value: f32 = conf_str
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0.7);
            (&line[..conf_start], conf_value)
        } else {
            (line, 0.7) // Default confidence if not specified
        };

        // Split "KEY: value"
        if let Some(colon_pos) = key_value.find(':') {
            let key = key_value[..colon_pos].trim().to_lowercase();
            let value = key_value[colon_pos + 1..].trim().to_string();

            if !key.is_empty() && !value.is_empty() {
                conventions.push(NeoConvention {
                    key,
                    value,
                    confidence,
                    confirmed: false, // Must be confirmed by user
                });
            }
        }
    }

    Ok(conventions)
}

/// Calls Anthropic API specifically for convention detection.
async fn call_anthropic_for_conventions(
    prompt: &str,
    api_key: &str,
) -> Result<String, NeoError> {
    use serde::{Deserialize, Serialize};

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

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&Request {
            model: "claude-haiku-4-5-20251001".to_string(),
            max_tokens: 800,
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        })
        .send()
        .await
        .map_err(|e| NeoError::AiError(e.to_string()))?;

    let parsed: Response = response
        .json()
        .await
        .map_err(|e| NeoError::AiError(e.to_string()))?;

    Ok(parsed
        .content
        .into_iter()
        .find(|b| b.block_type == "text")
        .and_then(|b| b.text)
        .unwrap_or_default())
}
