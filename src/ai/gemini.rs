use super::{AiConfig, AiResponse, CodeSnippet};
use anyhow::{anyhow, Result};
use log::{debug, error};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct GeminiClient {
    client: Client,
    config: AiConfig,
    base_url: String,
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ResponseContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    text: String,
}

impl GeminiClient {
    pub fn new(config: AiConfig) -> Self {
        let client = Client::new();
        let base_url = "https://generativelanguage.googleapis.com/v1beta/models".to_string();
        
        Self {
            client,
            config,
            base_url,
        }
    }

    pub async fn generate_response(&self, prompt: String) -> Result<AiResponse> {
        if self.config.api_key.is_empty() {
            return Err(anyhow!("Gemini API key not configured"));
        }

        let url = format!(
            "{}/{}:generateContent?key={}",
            self.base_url, self.config.model, self.config.api_key
        );

        let request_body = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part { text: prompt }],
            }],
            generation_config: GenerationConfig {
                temperature: self.config.temperature,
                max_output_tokens: self.config.max_tokens,
            },
        };

        debug!("Sending request to Gemini API: {}", url);

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Gemini API error: {}", error_text);
            return Err(anyhow!("Gemini API error: {}", error_text));
        }

        let gemini_response: GeminiResponse = response.json().await?;

        if gemini_response.candidates.is_empty() {
            return Err(anyhow!("No response from Gemini API"));
        }

        let candidate = &gemini_response.candidates[0];
        if candidate.content.parts.is_empty() {
            return Err(anyhow!("Empty response from Gemini API"));
        }

        let content = &candidate.content.parts[0].text;
        let parsed_response = self.parse_response(content);

        Ok(parsed_response)
    }

    fn parse_response(&self, content: &str) -> AiResponse {
        let mut suggestions = Vec::new();
        let mut code_snippets = Vec::new();
        let mut clean_content = content.to_string();

        // Extract code blocks
        let code_block_regex = regex::Regex::new(r"```(\w+)?\n(.*?)\n```").unwrap();
        for cap in code_block_regex.captures_iter(content) {
            let language = cap.get(1).map_or("text".to_string(), |m| m.as_str().to_string());
            let code = cap.get(2).map_or("", |m| m.as_str()).to_string();
            
            if !code.trim().is_empty() {
                code_snippets.push(CodeSnippet::new(
                    language,
                    code,
                    "Generated code snippet".to_string(),
                ));
            }
        }

        // Remove code blocks from content
        clean_content = code_block_regex.replace_all(&clean_content, "").to_string();

        // Extract suggestions (lines starting with "Suggestion:" or "Try:")
        let suggestion_regex = regex::Regex::new(r"(?i)(?:suggestion|try):\s*(.+)").unwrap();
        for cap in suggestion_regex.captures_iter(&clean_content) {
            if let Some(suggestion) = cap.get(1) {
                suggestions.push(suggestion.as_str().trim().to_string());
            }
        }

        AiResponse {
            content: clean_content.trim().to_string(),
            suggestions,
            code_snippets,
            confidence: 0.8, // Default confidence
        }
    }

    pub async fn explain_command(&self, command: &str) -> Result<AiResponse> {
        let prompt = format!(
            "{}\n\nExplain this command: `{}`\n\nProvide:\n1. What it does\n2. Key options/flags\n3. Example usage\n4. Potential risks or considerations",
            self.config.system_prompt, command
        );

        self.generate_response(prompt).await
    }

    pub async fn generate_command(&self, description: &str) -> Result<AiResponse> {
        let prompt = format!(
            "{}\n\nGenerate a command to: {}\n\nProvide:\n1. The command with explanation\n2. Alternative approaches if applicable\n3. Safety considerations\n\nFormat code in markdown code blocks.",
            self.config.system_prompt, description
        );

        self.generate_response(prompt).await
    }

    pub async fn fix_error(&self, error: &str, context: Option<&str>) -> Result<AiResponse> {
        let context_str = context.map(|c| format!("\n\nContext: {}", c)).unwrap_or_default();
        
        let prompt = format!(
            "{}\n\nFix this error: {}{}\n\nProvide:\n1. Explanation of the error\n2. Solution steps\n3. Prevention tips\n\nFormat commands in markdown code blocks.",
            self.config.system_prompt, error, context_str
        );

        self.generate_response(prompt).await
    }

    pub async fn review_code(&self, code: &str, language: Option<&str>) -> Result<AiResponse> {
        let language_str = language.unwrap_or("unknown");
        
        let prompt = format!(
            "{}\n\nReview this {} code:\n\n```{}\n{}\n```\n\nProvide:\n1. Code quality assessment\n2. Potential issues\n3. Improvement suggestions\n4. Best practices",
            self.config.system_prompt, language_str, language_str, code
        );

        self.generate_response(prompt).await
    }

    pub async fn analyze_security(&self, code: &str, language: &str) -> Result<AiResponse> {
        let prompt = format!(
            "{}\n\nPerform security analysis on this {} code:\n\n```{}\n{}\n```\n\nFocus on:\n1. Security vulnerabilities\n2. Potential attack vectors\n3. Recommended fixes\n4. Security best practices\n\nBe specific and actionable.",
            self.config.system_prompt, language, language, code
        );

        self.generate_response(prompt).await
    }

    pub fn update_config(&mut self, config: AiConfig) {
        self.config = config;
    }
}
