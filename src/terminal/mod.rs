pub mod block;
pub mod engine;
pub mod history;
pub mod pty;

pub use block::{Block, CommandBlock};
pub use engine::TerminalEngine;
pub use pty::PtyManager;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub shell: String,
    pub font_size: f32,
    pub theme: String,
    pub max_history: usize,
    pub enable_vi_mode: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: if cfg!(windows) {
                "pwsh".to_string()
            } else {
                "bash".to_string()
            },
            font_size: 14.0,
            theme: "dark".to_string(),
            max_history: 1000,
            enable_vi_mode: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    CommandStarted {
        id: Uuid,
        command: String,
    },
    CommandOutput {
        id: Uuid,
        output: String,
        is_stderr: bool,
    },
    CommandFinished {
        id: Uuid,
        exit_code: i32,
    },
    NewBlock {
        block: Block,
    },
    Error {
        message: String,
    },
}

pub type TerminalEventSender = mpsc::UnboundedSender<TerminalEvent>;
pub type TerminalEventReceiver = mpsc::UnboundedReceiver<TerminalEvent>;

#[derive(Debug, Clone)]
pub struct TerminalSession {
    pub id: Uuid,
    pub blocks: Vec<Block>,
    pub current_directory: String,
    pub environment: HashMap<String, String>,
    pub is_active: bool,
}

impl TerminalSession {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            blocks: Vec::new(),
            current_directory: std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            environment: std::env::vars().collect(),
            is_active: true,
        }
    }

    pub fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }

    pub fn get_last_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

    pub fn get_block_by_id(&self, id: &Uuid) -> Option<&Block> {
        self.blocks.iter().find(|b| &b.id == id)
    }
}
