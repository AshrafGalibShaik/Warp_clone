use super::{
    AiConfig, AiRequest, AiResponse, ChatMessage,
    GeminiClient
};
use super::chat::ChatSessionManager;
use anyhow::Result;
use log::{debug, error, info};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AiAgent {
    gemini_client: GeminiClient,
    chat_manager: Arc<RwLock<ChatSessionManager>>,
    config: AiConfig,
}

impl AiAgent {
    pub fn new(config: AiConfig) -> Self {
        let gemini_client = GeminiClient::new(config.clone());
        let chat_manager = Arc::new(RwLock::new(ChatSessionManager::new()));

        Self {
            gemini_client,
            chat_manager,
            config,
        }
    }

    pub async fn process_request(&self, request: AiRequest) -> Result<AiResponse> {
        debug!("Processing AI request: {:?}", request);

        match request {
            AiRequest::ExplainCommand { command } => {
                self.explain_command(&command).await
            }
            AiRequest::GenerateCommand { description } => {
                self.generate_command(&description).await
            }
            AiRequest::FixError { error, context } => {
                self.fix_error(&error, context.as_deref()).await
            }
            AiRequest::CodeReview { code, language } => {
                self.review_code(&code, language.as_deref()).await
            }
            AiRequest::SecurityAnalysis { code, language } => {
                self.analyze_security(&code, &language).await
            }
            AiRequest::Chat { message } => {
                self.handle_chat_message(&message).await
            }
        }
    }

    async fn explain_command(&self, command: &str) -> Result<AiResponse> {
        info!("Explaining command: {}", command);
        
        // Add to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.create_default_session_if_needed();
            chat_manager.add_message_to_active(ChatMessage::user(
                format!("Explain this command: {}", command)
            ));
        }

        let response = self.gemini_client.explain_command(command).await?;

        // Add response to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.add_message_to_active(ChatMessage::assistant(
                response.content.clone()
            ));
        }

        Ok(response)
    }

    async fn generate_command(&self, description: &str) -> Result<AiResponse> {
        info!("Generating command for: {}", description);

        // Add to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.create_default_session_if_needed();
            chat_manager.add_message_to_active(ChatMessage::user(
                format!("Generate a command to: {}", description)
            ));
        }

        let response = self.gemini_client.generate_command(description).await?;

        // Add response to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.add_message_to_active(ChatMessage::assistant(
                response.content.clone()
            ));
        }

        Ok(response)
    }

    async fn fix_error(&self, error: &str, context: Option<&str>) -> Result<AiResponse> {
        info!("Fixing error: {}", error);

        // Add to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.create_default_session_if_needed();
            
            let message = if let Some(ctx) = context {
                format!("Fix this error: {}\nContext: {}", error, ctx)
            } else {
                format!("Fix this error: {}", error)
            };
            
            chat_manager.add_message_to_active(ChatMessage::user(message));
        }

        let response = self.gemini_client.fix_error(error, context).await?;

        // Add response to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.add_message_to_active(ChatMessage::assistant(
                response.content.clone()
            ));
        }

        Ok(response)
    }

    async fn review_code(&self, code: &str, language: Option<&str>) -> Result<AiResponse> {
        let lang_str = language.unwrap_or("unknown");
        info!("Reviewing {} code", lang_str);

        // Add to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.create_default_session_if_needed();
            chat_manager.add_message_to_active(ChatMessage::user(
                format!("Review this {} code:\n\n```{}\n{}\n```", lang_str, lang_str, code)
            ));
        }

        let response = self.gemini_client.review_code(code, language).await?;

        // Add response to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.add_message_to_active(ChatMessage::assistant(
                response.content.clone()
            ));
        }

        Ok(response)
    }

    async fn analyze_security(&self, code: &str, language: &str) -> Result<AiResponse> {
        info!("Analyzing security for {} code", language);

        // Add to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.create_default_session_if_needed();
            chat_manager.add_message_to_active(ChatMessage::user(
                format!("Analyze security of this {} code:\n\n```{}\n{}\n```", language, language, code)
            ));
        }

        let response = self.gemini_client.analyze_security(code, language).await?;

        // Add response to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.add_message_to_active(ChatMessage::assistant(
                response.content.clone()
            ));
        }

        Ok(response)
    }

    async fn handle_chat_message(&self, message: &str) -> Result<AiResponse> {
        info!("Handling chat message");

        // Add user message to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.create_default_session_if_needed();
            chat_manager.add_message_to_active(ChatMessage::user(message.to_string()));
        }

        // Get conversation context
        let context = {
            let chat_manager = self.chat_manager.read().await;
            if let Some(session) = chat_manager.get_active_session() {
                session.get_context_for_ai(10) // Get last 10 messages for context
                    .into_iter()
                    .map(|msg| format!("{:?}: {}", msg.role, msg.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                String::new()
            }
        };

        // Create prompt with context
        let prompt = if context.is_empty() {
            format!("{}\n\nUser: {}", self.config.system_prompt, message)
        } else {
            format!(
                "{}\n\nConversation history:\n{}\n\nUser: {}",
                self.config.system_prompt, context, message
            )
        };

        let response = self.gemini_client.generate_response(prompt).await?;

        // Add response to chat history
        {
            let mut chat_manager = self.chat_manager.write().await;
            chat_manager.add_message_to_active(ChatMessage::assistant(
                response.content.clone()
            ));
        }

        Ok(response)
    }

    pub async fn create_chat_session(&self, title: String) -> uuid::Uuid {
        let mut chat_manager = self.chat_manager.write().await;
        chat_manager.create_session(title)
    }

    pub async fn switch_chat_session(&self, session_id: uuid::Uuid) -> bool {
        let mut chat_manager = self.chat_manager.write().await;
        chat_manager.switch_session(session_id)
    }

    pub async fn delete_chat_session(&self, session_id: uuid::Uuid) -> bool {
        let mut chat_manager = self.chat_manager.write().await;
        chat_manager.delete_session(session_id)
    }

    pub async fn get_chat_sessions(&self) -> Vec<(uuid::Uuid, String, chrono::DateTime<chrono::Utc>)> {
        let chat_manager = self.chat_manager.read().await;
        chat_manager
            .get_all_sessions()
            .iter()
            .map(|s| (s.id, s.title.clone(), s.updated_at))
            .collect()
    }

    pub async fn get_active_chat_messages(&self) -> Vec<ChatMessage> {
        let chat_manager = self.chat_manager.read().await;
        if let Some(session) = chat_manager.get_active_session() {
            session.get_messages().iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub async fn clear_active_chat(&self) {
        let mut chat_manager = self.chat_manager.write().await;
        if let Some(session) = chat_manager.get_active_session_mut() {
            session.clear_messages();
        }
    }

    pub async fn export_chat_to_markdown(&self) -> Option<String> {
        let chat_manager = self.chat_manager.read().await;
        chat_manager.get_active_session().map(|s| s.export_to_markdown())
    }

    pub fn update_config(&mut self, config: AiConfig) {
        self.config = config.clone();
        self.gemini_client.update_config(config);
    }

    pub fn get_config(&self) -> &AiConfig {
        &self.config
    }

    // Quick command suggestions based on context
    pub async fn suggest_commands(&self, current_directory: &str, recent_commands: &[String]) -> Result<Vec<String>> {
        let context = format!(
            "Current directory: {}\nRecent commands: {}",
            current_directory,
            recent_commands.join(", ")
        );

        let prompt = format!(
            "{}\n\nBased on this context: {}\n\nSuggest 5 useful commands the user might want to run next. Return only the commands, one per line.",
            self.config.system_prompt, context
        );

        match self.gemini_client.generate_response(prompt).await {
            Ok(response) => {
                let suggestions = response.content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.trim().to_string())
                    .take(5)
                    .collect();
                Ok(suggestions)
            }
            Err(e) => {
                error!("Failed to generate command suggestions: {}", e);
                Ok(vec![
                    "ls".to_string(),
                    "pwd".to_string(),
                    "git status".to_string(),
                    "clear".to_string(),
                ])
            }
        }
    }
}
