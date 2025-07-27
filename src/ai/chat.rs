use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

impl ChatMessage {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    pub fn user(content: String) -> Self {
        Self::new(MessageRole::User, content)
    }

    pub fn assistant(content: String) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    pub fn system(content: String) -> Self {
        Self::new(MessageRole::System, content)
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%H:%M:%S").to_string()
    }
}

#[derive(Debug)]
pub struct ChatSession {
    pub id: Uuid,
    pub title: String,
    pub messages: VecDeque<ChatMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub max_messages: usize,
}

impl ChatSession {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            messages: VecDeque::new(),
            created_at: now,
            updated_at: now,
            max_messages: 100,
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push_back(message);
        self.updated_at = Utc::now();

        // Maintain max message limit
        while self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    pub fn get_messages(&self) -> &VecDeque<ChatMessage> {
        &self.messages
    }

    pub fn get_recent_messages(&self, count: usize) -> Vec<&ChatMessage> {
        self.messages.iter().rev().take(count).collect()
    }

    pub fn get_context_for_ai(&self, max_messages: usize) -> Vec<&ChatMessage> {
        // Get recent messages for AI context, excluding system messages
        self.messages
            .iter()
            .rev()
            .filter(|msg| !matches!(msg.role, MessageRole::System))
            .take(max_messages)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }

    pub fn export_to_markdown(&self) -> String {
        let mut markdown = format!(
            "# Chat Session: {}\n\nCreated: {}\nUpdated: {}\n\n",
            self.title,
            self.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
            self.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
        );

        for message in &self.messages {
            let role_icon = match message.role {
                MessageRole::User => "👤",
                MessageRole::Assistant => "🤖",
                MessageRole::System => "⚙️",
            };

            markdown.push_str(&format!(
                "## {} {:?} - {}\n\n{}\n\n",
                role_icon,
                message.role,
                message.formatted_timestamp(),
                message.content
            ));
        }

        markdown
    }

    pub fn search_messages(&self, query: &str) -> Vec<&ChatMessage> {
        self.messages
            .iter()
            .filter(|msg| msg.content.to_lowercase().contains(&query.to_lowercase()))
            .collect()
    }

    pub fn get_message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn get_user_message_count(&self) -> usize {
        self.messages
            .iter()
            .filter(|msg| matches!(msg.role, MessageRole::User))
            .count()
    }

    pub fn get_assistant_message_count(&self) -> usize {
        self.messages
            .iter()
            .filter(|msg| matches!(msg.role, MessageRole::Assistant))
            .count()
    }
}

#[derive(Debug)]
pub struct ChatSessionManager {
    sessions: Vec<ChatSession>,
    active_session_id: Option<Uuid>,
    max_sessions: usize,
}

impl ChatSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            active_session_id: None,
            max_sessions: 10,
        }
    }

    pub fn create_session(&mut self, title: String) -> Uuid {
        let session = ChatSession::new(title);
        let session_id = session.id;

        self.sessions.push(session);

        // Maintain max sessions
        while self.sessions.len() > self.max_sessions {
            self.sessions.remove(0);
        }

        // Set as active if it's the first session
        if self.active_session_id.is_none() {
            self.active_session_id = Some(session_id);
        }

        session_id
    }

    pub fn get_active_session(&self) -> Option<&ChatSession> {
        if let Some(active_id) = self.active_session_id {
            self.sessions.iter().find(|s| s.id == active_id)
        } else {
            None
        }
    }

    pub fn get_active_session_mut(&mut self) -> Option<&mut ChatSession> {
        if let Some(active_id) = self.active_session_id {
            self.sessions.iter_mut().find(|s| s.id == active_id)
        } else {
            None
        }
    }

    pub fn switch_session(&mut self, session_id: Uuid) -> bool {
        if self.sessions.iter().any(|s| s.id == session_id) {
            self.active_session_id = Some(session_id);
            true
        } else {
            false
        }
    }

    pub fn delete_session(&mut self, session_id: Uuid) -> bool {
        if let Some(index) = self.sessions.iter().position(|s| s.id == session_id) {
            self.sessions.remove(index);

            // If this was the active session, switch to another one
            if Some(session_id) == self.active_session_id {
                self.active_session_id = self.sessions.first().map(|s| s.id);
            }

            true
        } else {
            false
        }
    }

    pub fn get_all_sessions(&self) -> &[ChatSession] {
        &self.sessions
    }

    pub fn add_message_to_active(&mut self, message: ChatMessage) {
        if let Some(session) = self.get_active_session_mut() {
            session.add_message(message);
        }
    }

    pub fn create_default_session_if_needed(&mut self) {
        if self.sessions.is_empty() {
            let session_id = self.create_session("Default Chat".to_string());
            self.active_session_id = Some(session_id);
        }
    }
}
