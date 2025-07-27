use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockType {
    Command,
    Output,
    Error,
    System,
    AiResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: Uuid,
    pub block_type: BlockType,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
    pub is_collapsible: bool,
    pub is_collapsed: bool,
    pub exit_code: Option<i32>,
    pub execution_time: Option<u64>, // milliseconds
}

impl Block {
    pub fn new(block_type: BlockType, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            block_type,
            content,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            is_collapsible: false,
            is_collapsed: false,
            exit_code: None,
            execution_time: None,
        }
    }

    pub fn command(content: String) -> Self {
        let mut block = Self::new(BlockType::Command, content);
        block.is_collapsible = true;
        block
    }

    pub fn output(content: String) -> Self {
        Self::new(BlockType::Output, content)
    }

    pub fn error(content: String) -> Self {
        Self::new(BlockType::Error, content)
    }

    pub fn system(content: String) -> Self {
        Self::new(BlockType::System, content)
    }

    pub fn ai_response(content: String) -> Self {
        let mut block = Self::new(BlockType::AiResponse, content);
        block.is_collapsible = true;
        block
    }

    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    pub fn set_execution_time(&mut self, duration_ms: u64) {
        self.execution_time = Some(duration_ms);
    }

    pub fn set_exit_code(&mut self, code: i32) {
        self.exit_code = Some(code);
    }

    pub fn toggle_collapsed(&mut self) {
        if self.is_collapsible {
            self.is_collapsed = !self.is_collapsed;
        }
    }

    pub fn is_success(&self) -> bool {
        match self.exit_code {
            Some(0) => true,
            Some(_) => false,
            None => true, // No exit code means it's not a command that failed
        }
    }

    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%H:%M:%S").to_string()
    }

    pub fn formatted_execution_time(&self) -> Option<String> {
        self.execution_time.map(|ms| {
            if ms < 1000 {
                format!("{}ms", ms)
            } else if ms < 60000 {
                format!("{:.1}s", ms as f64 / 1000.0)
            } else {
                format!("{}m {:02}s", ms / 60000, (ms % 60000) / 1000)
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct CommandBlock {
    pub command_block: Block,
    pub output_blocks: Vec<Block>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub working_directory: String,
}

impl CommandBlock {
    pub fn new(command: String, working_directory: String) -> Self {
        Self {
            command_block: Block::command(command),
            output_blocks: Vec::new(),
            start_time: Utc::now(),
            end_time: None,
            working_directory,
        }
    }

    pub fn add_output(&mut self, content: String, is_stderr: bool) {
        let block = if is_stderr {
            Block::error(content)
        } else {
            Block::output(content)
        };
        self.output_blocks.push(block);
    }

    pub fn finish(&mut self, exit_code: i32) {
        self.end_time = Some(Utc::now());
        self.command_block.set_exit_code(exit_code);
        
        if let Some(end_time) = self.end_time {
            let duration = (end_time - self.start_time).num_milliseconds() as u64;
            self.command_block.set_execution_time(duration);
        }
    }

    pub fn get_all_blocks(&self) -> Vec<&Block> {
        let mut blocks = vec![&self.command_block];
        blocks.extend(self.output_blocks.iter());
        blocks
    }

    pub fn get_combined_output(&self) -> String {
        self.output_blocks
            .iter()
            .map(|b| b.content.as_str())
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn is_running(&self) -> bool {
        self.end_time.is_none()
    }
}
