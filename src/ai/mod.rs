pub mod agent;
pub mod chat;
pub mod gemini;

use serde::{Deserialize, Serialize};

pub use agent::AiAgent;
pub use chat::ChatMessage;
pub use gemini::GeminiClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub system_prompt: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            model: "gemini-2.0-flash".to_string(),
            max_tokens: 2048,
            temperature: 0.7,
            system_prompt: "You are an AI assistant integrated into ANTRAFT, a modern terminal application. You help users with command-line tasks, explain commands, suggest solutions, and provide coding assistance. Be concise but helpful.".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AiRequest {
    ExplainCommand {
        command: String,
    },
    GenerateCommand {
        description: String,
    },
    FixError {
        error: String,
        context: Option<String>,
    },
    CodeReview {
        code: String,
        language: Option<String>,
    },
    SecurityAnalysis {
        code: String,
        language: String,
    },
    Chat {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct AiResponse {
    pub content: String,
    pub suggestions: Vec<String>,
    pub code_snippets: Vec<CodeSnippet>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub language: String,
    pub code: String,
    pub description: String,
    pub can_execute: bool,
}

impl CodeSnippet {
    pub fn new(language: String, code: String, description: String) -> Self {
        let can_execute = matches!(language.as_str(), "bash" | "sh" | "powershell" | "cmd");
        Self {
            language,
            code,
            description,
            can_execute,
        }
    }
}
