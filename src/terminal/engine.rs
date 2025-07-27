use super::{
    Block, CommandBlock, TerminalConfig, TerminalEvent, 
    TerminalEventSender, TerminalSession, PtyManager
};
use anyhow::{anyhow, Result};
use log::{debug, error, info};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct TerminalEngine {
    config: TerminalConfig,
    sessions: Arc<RwLock<HashMap<Uuid, TerminalSession>>>,
    active_session_id: Arc<RwLock<Option<Uuid>>>,
    event_sender: TerminalEventSender,
    pty_manager: Arc<PtyManager>,
    is_running: Arc<AtomicBool>,
}

impl TerminalEngine {
    pub fn new(config: TerminalConfig, event_sender: TerminalEventSender) -> Result<Self> {
        let pty_manager = Arc::new(PtyManager::new()?);
        
        Ok(Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            active_session_id: Arc::new(RwLock::new(None)),
            event_sender,
            pty_manager,
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    pub async fn create_session(&self) -> Result<Uuid> {
        let session = TerminalSession::new();
        let session_id = session.id;
        
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id, session);
        }
        
        {
            let mut active_id = self.active_session_id.write().await;
            if active_id.is_none() {
                *active_id = Some(session_id);
            }
        }
        
        info!("Created new terminal session: {}", session_id);
        Ok(session_id)
    }

    pub async fn get_active_session(&self) -> Option<TerminalSession> {
        let active_id = self.active_session_id.read().await;
        if let Some(id) = *active_id {
            let sessions = self.sessions.read().await;
            sessions.get(&id).cloned()
        } else {
            None
        }
    }

    pub async fn switch_session(&self, session_id: Uuid) -> Result<()> {
        let sessions = self.sessions.read().await;
        if sessions.contains_key(&session_id) {
            let mut active_id = self.active_session_id.write().await;
            *active_id = Some(session_id);
            info!("Switched to session: {}", session_id);
            Ok(())
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn execute_command(&self, command: String) -> Result<Uuid> {
        let session_id = match *self.active_session_id.read().await {
            Some(id) => id,
            None => self.create_session().await?,
        };

        let working_directory = {
            let sessions = self.sessions.read().await;
            sessions.get(&session_id)
                .map(|s| s.current_directory.clone())
                .unwrap_or_else(|| std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string())
        };

        let command_block = CommandBlock::new(command.clone(), working_directory.clone());
        let command_id = command_block.command_block.id;

        // Add command block to session
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.add_block(command_block.command_block.clone());
            }
        }

        // Send command started event
        let _ = self.event_sender.send(TerminalEvent::CommandStarted {
            id: command_id,
            command: command.clone(),
        });

        // Execute the command asynchronously
        let event_sender = self.event_sender.clone();
        let sessions = self.sessions.clone();
        let shell = self.config.shell.clone();
        
        tokio::spawn(async move {
            let result = Self::run_command_async(
                command,
                working_directory,
                shell,
                command_id,
                event_sender.clone(),
                sessions,
                session_id,
            ).await;

            if let Err(e) = result {
                error!("Command execution failed: {}", e);
                let _ = event_sender.send(TerminalEvent::Error {
                    message: format!("Command execution failed: {}", e),
                });
            }
        });

        Ok(command_id)
    }

    async fn run_command_async(
        command: String,
        working_directory: String,
        shell: String,
        command_id: Uuid,
        event_sender: TerminalEventSender,
        _sessions: Arc<RwLock<HashMap<Uuid, TerminalSession>>>,
        _session_id: Uuid,
    ) -> Result<()> {
        debug!("Executing command: {} in {}", command, working_directory);

        let mut child = if cfg!(windows) {
            Command::new(&shell)
                .args(&["-Command", &command])
                .current_dir(&working_directory)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        } else {
            Command::new(&shell)
                .args(&["-c", &command])
                .current_dir(&working_directory)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        };

        // Handle stdout
        if let Some(stdout) = child.stdout.take() {
            let event_sender_stdout = event_sender.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = event_sender_stdout.send(TerminalEvent::CommandOutput {
                        id: command_id,
                        output: format!("{}\n", line),
                        is_stderr: false,
                    });
                }
            });
        }

        // Handle stderr
        if let Some(stderr) = child.stderr.take() {
            let event_sender_stderr = event_sender.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = event_sender_stderr.send(TerminalEvent::CommandOutput {
                        id: command_id,
                        output: format!("{}\n", line),
                        is_stderr: true,
                    });
                }
            });
        }

        // Wait for command to finish
        let exit_status = child.wait().await?;
        let exit_code = exit_status.code().unwrap_or(-1);

        // Send command finished event
        let _ = event_sender.send(TerminalEvent::CommandFinished {
            id: command_id,
            exit_code,
        });

        debug!("Command finished with exit code: {}", exit_code);
        Ok(())
    }

    pub async fn handle_command_output(&self, command_id: Uuid, output: String, is_stderr: bool) -> Result<()> {
        let sessions = self.sessions.clone();
        let mut sessions_guard = sessions.write().await;
        
        for session in sessions_guard.values_mut() {
            if let Some(block) = session.blocks.iter_mut().rev().find(|b| b.id == command_id) {
                // This is simplified - in a real implementation, you'd want to manage
                // command blocks more sophisticatedly
                if is_stderr {
                    let error_block = Block::error(output);
                    session.add_block(error_block);
                } else {
                    let output_block = Block::output(output);
                    session.add_block(output_block);
                }
                break;
            }
        }
        
        Ok(())
    }

    pub async fn handle_command_finished(&self, command_id: Uuid, exit_code: i32) -> Result<()> {
        let sessions = self.sessions.clone();
        let mut sessions_guard = sessions.write().await;
        
        for session in sessions_guard.values_mut() {
            if let Some(block) = session.blocks.iter_mut().rev().find(|b| b.id == command_id) {
                block.set_exit_code(exit_code);
                break;
            }
        }
        
        Ok(())
    }

    pub async fn get_session_blocks(&self, session_id: Uuid) -> Result<Vec<Block>> {
        let sessions = self.sessions.read().await;
        match sessions.get(&session_id) {
            Some(session) => Ok(session.blocks.clone()),
            None => Err(anyhow!("Session not found: {}", session_id)),
        }
    }

    pub async fn clear_session(&self, session_id: Uuid) -> Result<()> {
        let sessions = self.sessions.clone();
        let mut sessions_guard = sessions.write().await;
        
        if let Some(session) = sessions_guard.get_mut(&session_id) {
            session.blocks.clear();
            info!("Cleared session: {}", session_id);
            Ok(())
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn shutdown(&self) {
        info!("Shutting down terminal engine");
        self.is_running.store(false, Ordering::Relaxed);
        
        // Clean up sessions
        let mut sessions = self.sessions.write().await;
        sessions.clear();
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    // Built-in commands
    pub async fn handle_builtin_command(&self, command: &str) -> Option<Result<Block>> {
        match command.trim() {
            "clear" => {
                if let Some(session) = self.get_active_session().await {
                    if let Err(e) = self.clear_session(session.id).await {
                        return Some(Err(e));
                    }
                }
                Some(Ok(Block::system("Screen cleared".to_string())))
            }
            "exit" | "quit" => {
                self.shutdown().await;
                Some(Ok(Block::system("Goodbye!".to_string())))
            }
            cmd if cmd.starts_with("cd ") => {
                let path = cmd.strip_prefix("cd ").unwrap().trim();
                match std::env::set_current_dir(path) {
                    Ok(_) => {
                        let new_dir = std::env::current_dir()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        
                        // Update session directory
                        if let Some(active_id) = *self.active_session_id.read().await {
                            let mut sessions = self.sessions.write().await;
                            if let Some(session) = sessions.get_mut(&active_id) {
                                session.current_directory = new_dir.clone();
                            }
                        }
                        
                        Some(Ok(Block::system(format!("Changed directory to: {}", new_dir))))
                    }
                    Err(e) => Some(Err(anyhow!("Failed to change directory: {}", e))),
                }
            }
            "pwd" => {
                let current_dir = std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                Some(Ok(Block::output(current_dir)))
            }
            _ => None,
        }
    }
}