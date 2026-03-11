use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct GeminiResult {
    pub scores: Value,
    pub insights_markdown: String,
}

const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

fn build_prompt(events_summary: &str, num_screenshots: usize) -> String {
    let screenshot_instruction = if num_screenshots > 0 {
        format!(
            "\nYou are also given {} screenshots from the session (sampled every 30s).\n\
             Use them to understand:\n\
             - What tools/editors/browsers the user was working in\n\
             - Whether they were using AI assistants (Claude, Cursor, Copilot, etc.)\n\
             - The quality and complexity of their work\n\
             - Any manual patterns (typing code by hand, copy-pasting from StackOverflow, etc.)\n\n\
             IMPORTANT: The screenshots are the ground truth. The local score data below is a rough heuristic estimate. \
             Your visual analysis of the screenshots should override the local scores where they disagree.\n",
            num_screenshots
        )
    } else {
        String::new()
    };

    format!(
        r#"You are the AI scoring engine and coach for AgentELO, a platform that tracks how AI-natively people work.

AgentELO captures workflow data (window focus, keystrokes, file changes, terminal commands, AI processes, screenshots) and scores how effectively the user leverages AI.

The session could be ANY kind of work — coding, writing, legal, design, research, etc.

Local Score Data (heuristic estimate): {{}}

Events:
{events_summary}
{screenshot_instruction}
You MUST respond with valid JSON in exactly this format — no markdown, no code fences, just raw JSON:

{{
  "scores": {{
    "delegation": <0-100>,
    "iteration": <0-100>,
    "parallelism": <0-100>,
    "independence": <0-100>,
    "shipping": <0-100>,
    "overall": <0-100>
  }},
  "insights": {{
    "summary": "<2-3 sentences: what the user actually did>",
    "ai_assessment": "<how effectively they used AI>",
    "missed_opportunities": ["<specific moment 1>", "<specific moment 2>"],
    "tips": ["<actionable tip 1>", "<actionable tip 2>", "<actionable tip 3>"],
    "badges": ["<badge1>", "<badge2>"]
  }}
}}

Scoring dimensions:
- delegation (30% weight): Did they make AI do the work? Look at screenshots — is AI generating code/content, or is the user typing manually? Time spent in AI tools vs manual work.
- iteration (20% weight): Did they refine with AI? Multiple back-and-forth exchanges, not just one-shot. Evidence of re-prompting, adjusting AI output.
- parallelism (20% weight): Multiple AI tasks running at once? Multiple Claude/Cursor windows, concurrent AI processes, working across multiple projects.
- independence (15% weight): Did they avoid manual fallbacks? Going to AI instead of StackOverflow/Google for answers. Not hand-typing what AI could generate.
- shipping (15% weight): Did they produce output? File changes, commits, actual work product — not just chatting with AI.

Score based on what you SEE in the screenshots and events, not just the local heuristic data.

Badge options: AI Collaborator, Prompt Engineer, Speed Demon, Zero Manual, AI Native, Strategic AI User, Multi-Tool Master

Do NOT recommend specific AI tools. Focus on workflow patterns."#
    )
}

fn strip_code_fences(raw: &str) -> &str {
    let mut s = raw.trim();
    if s.starts_with("```") {
        if let Some(pos) = s[3..].find('\n') {
            s = &s[3 + pos + 1..];
        } else {
            s = &s[3..];
        }
    }
    if s.ends_with("```") {
        s = &s[..s.len() - 3];
    }
    s.trim()
}

fn insights_to_markdown(insights: &Value) -> String {
    let mut md = String::new();

    if let Some(summary) = insights.get("summary").and_then(|v| v.as_str()) {
        md.push_str(&format!("## Summary\n{}\n\n", summary));
    }
    if let Some(assessment) = insights.get("ai_assessment").and_then(|v| v.as_str()) {
        md.push_str(&format!("## AI Assessment\n{}\n\n", assessment));
    }
    if let Some(missed) = insights.get("missed_opportunities").and_then(|v| v.as_array()) {
        md.push_str("## Missed Opportunities\n");
        for item in missed {
            if let Some(s) = item.as_str() {
                md.push_str(&format!("- {}\n", s));
            }
        }
        md.push('\n');
    }
    if let Some(tips) = insights.get("tips").and_then(|v| v.as_array()) {
        md.push_str("## Tips\n");
        for item in tips {
            if let Some(s) = item.as_str() {
                md.push_str(&format!("- {}\n", s));
            }
        }
        md.push('\n');
    }

    md
}

pub async fn score_session_locally(
    api_key: &str,
    events_summary: &str,
    screenshot_frames: &[String],
) -> Result<GeminiResult, String> {
    let prompt = build_prompt(events_summary, screenshot_frames.len());

    // Build multimodal parts: text + inline images
    let mut parts = Vec::new();
    parts.push(serde_json::json!({"text": prompt}));

    for frame in screenshot_frames {
        parts.push(serde_json::json!({
            "inline_data": {
                "mime_type": "image/jpeg",
                "data": frame,
            }
        }));
    }

    let body = serde_json::json!({
        "contents": [{"parts": parts}],
        "generationConfig": {"maxOutputTokens": 2000}
    });

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp = client
        .post(GEMINI_API_URL)
        .header("x-goog-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini API error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini API {status}: {body}"));
    }

    let data: Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;

    let raw_text = data
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or("No text in Gemini response")?;

    let cleaned = strip_code_fences(raw_text);
    let parsed: Value =
        serde_json::from_str(cleaned).map_err(|e| format!("Invalid JSON from Gemini: {e}"))?;

    let scores = parsed
        .get("scores")
        .cloned()
        .ok_or("No 'scores' in Gemini response")?;

    let insights_markdown = parsed
        .get("insights")
        .map(insights_to_markdown)
        .unwrap_or_default();

    Ok(GeminiResult {
        scores,
        insights_markdown,
    })
}
