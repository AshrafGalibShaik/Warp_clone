use crate::ai::{AiAgent, AiConfig, AiRequest, AiResponse};
use crate::autocomplete::{AutocompleteContext, AutocompleteEngine};
use crate::file_explorer::FileExplorer;
use crate::security::{ScanType, SecurityConfig, SecurityScanRequest, SecurityScanner};
use crate::terminal::{TerminalEngine, TerminalEventSender};
use anyhow::Result;
use crossbeam_channel;
use eframe::egui;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::runtime::Handle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ai: AiConfig,
    pub security: SecurityConfig,
    pub terminal: crate::terminal::TerminalConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            security: SecurityConfig::default(),
            terminal: crate::terminal::TerminalConfig::default(),
        }
    }
}

pub struct AnTraftApp {
    config: Config,
    terminal_engine: Arc<TerminalEngine>,
    ai_agent: Arc<RwLock<AiAgent>>,
    file_explorer: Arc<RwLock<FileExplorer>>,
    autocomplete_engine: Arc<RwLock<AutocompleteEngine>>,
    security_scanner: Arc<SecurityScanner>,
    terminal_event_tx: TerminalEventSender,
    pub response_sender: crossbeam_channel::Sender<AiResponse>,
    pub response_receiver: crossbeam_channel::Receiver<AiResponse>,
    // UI State
    current_mode: UIMode,
    command_input: String,
    command_history: VecDeque<String>,
    terminal_output: Vec<TerminalBlock>,
    ai_input: String,
    ai_messages: Vec<(String, String)>, // (role, message)
    runtime_handle: Handle,
}

#[derive(Debug, Clone)]
pub struct TerminalBlock {
    pub id: uuid::Uuid,
    pub command: String,
    pub output: String,
    pub is_running: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq)]
enum UIMode {
    Welcome,
    Terminal,
    AiAgent,
}


impl AnTraftApp {
    pub async fn new(config: Config) -> Result<Self> {
        let (terminal_event_tx, _terminal_event_rx) = tokio::sync::mpsc::unbounded_channel();

        let terminal_engine =
            TerminalEngine::new(config.terminal.clone(), terminal_event_tx.clone())?;
        let ai_agent = Arc::new(RwLock::new(AiAgent::new(config.ai.clone())));
        let file_explorer = Arc::new(RwLock::new(FileExplorer::new(std::env::current_dir()?)?));
        let autocomplete_engine = Arc::new(RwLock::new(AutocompleteEngine::new()));
        let security_scanner = Arc::new(SecurityScanner::new(config.security.clone())?);

        let (response_sender, response_receiver) = crossbeam_channel::unbounded();

        let runtime_handle = Handle::current();

        let app = AnTraftApp {
            config,
            terminal_engine: Arc::new(terminal_engine),
            ai_agent,
            file_explorer,
            autocomplete_engine,
            security_scanner,
            terminal_event_tx,
            response_sender,
            response_receiver,
            // Initialize UI state
            current_mode: UIMode::Welcome,
            command_input: String::new(),
            command_history: VecDeque::new(),
            terminal_output: Vec::new(),
            ai_input: String::new(),
            ai_messages: Vec::new(),
            runtime_handle,
        };

        Ok(app)
    }

    pub async fn run_security_scan(&self, path: String, scan_type: ScanType) -> Result<()> {
        let request = SecurityScanRequest {
            path: path.into(),
            scan_type,
            include_patterns: vec![],
            exclude_patterns: vec![],
        };

        let report = self.security_scanner.scan(request).await?;

        // Handle the report generation and display
        let markdown_report = report.to_markdown();
        println!("Security Report:\n{}", markdown_report);

        Ok(())
    }

    // Only keep the async version of execute_terminal_command
    pub async fn execute_terminal_command(&self, command: String) -> Result<()> {
        let response_tx = self.response_sender.clone();

        // Execute command
        self.terminal_engine
            .execute_command(command.clone())
            .await?;

        // Example of using AI agent after executing the command
        let ai_agent = self.ai_agent.clone();
        tokio::spawn(async move {
            let response = ai_agent
                .read()
                .await
                .process_request(AiRequest::ExplainCommand { command })
                .await;
            if let Ok(ai_response) = response {
                let _ = response_tx.send(ai_response);
            }
        });

        Ok(())
    }

    pub async fn perform_autocomplete(
        &self,
        input: String,
        context: AutocompleteContext,
    ) -> Result<Vec<String>> {
        let engine = self.autocomplete_engine.read().await;
        let suggestions = engine.get_suggestions(&input, &context);
        Ok(suggestions.into_iter().map(|s| s.insert_text).collect())
    }

    // UI helpers (not trait methods)
    pub fn render_ai_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("🤖 AI Assistant");
        ui.separator();
        
        // Chat history
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for (role, message) in &self.ai_messages {
                    ui.group(|ui| {
                        let color = if role == "You" {
                            egui::Color32::from_rgb(100, 150, 255)
                        } else {
                            egui::Color32::from_rgb(100, 255, 150)
                        };
                        ui.colored_label(color, format!("{}: ", role));
                        ui.label(message);
                    });
                    ui.add_space(5.0);
                }
            });
        
        ui.separator();
        
        // Input area
        ui.horizontal(|ui| {
            let response = ui.text_edit_singleline(&mut self.ai_input);
            
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !self.ai_input.is_empty() {
                    self.send_ai_message();
                }
            }
            
            if ui.button("Send").clicked() && !self.ai_input.is_empty() {
                self.send_ai_message();
            }
        });
        
        ui.separator();
        ui.small("💡 Try asking: 'Explain the last command', 'Help with git', 'Debug this error'");
    }

    pub fn render_terminal(&mut self, ui: &mut egui::Ui) {
        // Warp-like terminal interface
        ui.vertical(|ui| {
            // Terminal output area (scrollable)
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    // Show command history and outputs
                    for block in &self.terminal_output {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(egui::Color32::from_rgb(100, 200, 100), ">");
                                ui.label(&block.command);
                                if block.is_running {
                                    ui.spinner();
                                }
                            });
                            if !block.output.is_empty() {
                                ui.separator();
                                ui.label(&block.output);
                            }
                        });
                        ui.add_space(5.0);
                    }
                });

            ui.separator();
            
            // Command input area at bottom (like Warp)
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(100, 200, 100), "❯");
                let response = ui.text_edit_singleline(&mut self.command_input);
                
                // Auto-focus the input field
                response.request_focus();
                
                // Handle Enter key to execute command
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !self.command_input.is_empty() {
                        self.execute_command_sync();
                    }
                }
                
                if ui.button("⚡ Run").clicked() && !self.command_input.is_empty() {
                    self.execute_command_sync();
                }
            });
        });
    }

    fn execute_command_sync(&mut self) {
        let command = self.command_input.trim().to_string();
        if command.is_empty() {
            return;
        }

        // Add command to history
        self.command_history.push_front(command.clone());
        
        // Create terminal block
        let block_id = uuid::Uuid::new_v4();
        let mut block = TerminalBlock {
            id: block_id,
            command: command.clone(),
            output: String::new(),
            is_running: true,
            timestamp: chrono::Utc::now(),
        };
        
        // Execute command and capture output
        let output = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(["/C", &command])
                .output()
        } else {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
        };

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                
                let combined_output = if !stderr.is_empty() {
                    format!("{}\n{}", stdout, stderr)
                } else {
                    stdout.to_string()
                };
                
                block.output = combined_output;
                block.is_running = false;
            }
            Err(e) => {
                block.output = format!("Error executing command: {}", e);
                block.is_running = false;
            }
        }
        
        self.terminal_output.push(block);
        self.command_input.clear();
    }

    pub fn render_file_explorer(&mut self, ui: &mut egui::Ui) {
        ui.heading("File Explorer");
        // Add your file explorer UI code here
    }

    pub fn render_security_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Security Panel");
        // Add your security panel UI code here
    }

    pub fn send_ai_message(&mut self) {
        if self.ai_input.is_empty() {
            return;
        }

        let message = self.ai_input.clone();
        self.ai_messages.push(("You".to_string(), message.clone()));
        self.ai_input.clear();

        // Add a placeholder for the AI response that will be updated
        self.ai_messages.push(("AI".to_string(), "🤔 Thinking...".to_string()));

        // Process the message with the AI agent asynchronously
        let ai_agent = self.ai_agent.clone();
        let runtime_handle = self.runtime_handle.clone();
        let response_sender = self.response_sender.clone();
        let _ai_message_index = self.ai_messages.len() - 1;

        runtime_handle.spawn(async move {
            // Create an AI request based on the user's message
            let ai_request = AiRequest::Chat { message: message.clone() };
            
            // Process the request with the AI agent
            match ai_agent.read().await.process_request(ai_request).await {
                Ok(ai_response) => {
                    // Send the response back to the UI thread
                    let _ = response_sender.send(ai_response);
                }
                Err(e) => {
                    // Send error response
                    let error_response = AiResponse {
                        content: format!("Sorry, I encountered an error: {}", e),
                        confidence: 0.0,
                        suggestions: vec![],
                        code_snippets: vec![],
                    };
                    let _ = response_sender.send(error_response);
                }
            }
        });
    }

    pub fn execute_command(&mut self) {
        if self.command_input.is_empty() {
            return;
        }

        let command = self.command_input.clone();
        self.command_history.push_front(command.clone());

        // Create a new terminal block
        let block = TerminalBlock {
            id: uuid::Uuid::new_v4(),
            command: command.clone(),
            output: String::new(),
            is_running: true,
            timestamp: chrono::Utc::now(),
        };

        self.terminal_output.push(block.clone());
        self.command_input.clear();

        // Execute the command
        let runtime_handle = self.runtime_handle.clone();
        let block_id = block.id;
        let mut output_blocks = self.terminal_output.clone();

        runtime_handle.spawn(async move {
            // Simulate command execution
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Update the block with output
            if let Some(block) = output_blocks.iter_mut().find(|b| b.id == block_id) {
                block.output = format!("Executed: {}", command);
                block.is_running = false;
            }
        });
    }

    pub fn start_security_scan(&mut self, scan_type: ScanType) {
        info!("Starting {:?} security scan", scan_type);
        // TODO: Implement actual security scan
    }

    fn render_welcome_screen(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                
                // Welcome heading
                ui.heading("Hello, Shaik!");
                ui.label("Get started with one of these suggestions");
                ui.add_space(30.0);
                
                // Action cards in a grid
                ui.horizontal(|ui| {
                    ui.add_space(50.0);
                    
                    // Install card
                    if self.render_action_card(ui, "⬇", "Install", "Install a binary/dependency") {
                        self.command_input = "npm install ".to_string();
                        self.current_mode = UIMode::Terminal;
                    }
                    
                    ui.add_space(20.0);
                    
                    // Code card
                    if self.render_action_card(ui, "</>", "Code", "Start a new project/feature or fix a bug") {
                        self.command_input = "code .".to_string();
                        self.current_mode = UIMode::Terminal;
                    }
                    
                    ui.add_space(20.0);
                    
                    // Deploy card
                    if self.render_action_card(ui, "🚀", "Deploy", "Deploy your project") {
                        self.command_input = "git push origin main".to_string();
                        self.current_mode = UIMode::Terminal;
                    }
                    
                    ui.add_space(20.0);
                    
                    // AI Agent card
                    if self.render_action_card(ui, "🤖", "Something else?", "Run with an Agent to accomplish another task") {
                        self.current_mode = UIMode::AiAgent;
                    }
                });
            });
            
            // Bottom command input
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(50.0);
                    ui.label("❯");
                    let response = ui.add_sized([600.0, 25.0], egui::TextEdit::singleline(&mut self.command_input)
                        .hint_text("code, ask, build, or run commands"));
                    
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !self.command_input.is_empty() {
                            if self.command_input.starts_with("ai ") || self.command_input.starts_with("ask ") {
                                self.ai_input = self.command_input.clone();
                                self.current_mode = UIMode::AiAgent;
                            } else {
                                self.current_mode = UIMode::Terminal;
                                self.execute_command_sync();
                            }
                        }
                    }
                });
                
                // Mode selector
                ui.horizontal(|ui| {
                    ui.add_space(100.0);
                    if ui.selectable_label(self.current_mode == UIMode::Terminal, "🖥 Terminal").clicked() {
                        self.current_mode = UIMode::Terminal;
                    }
                    if ui.selectable_label(self.current_mode == UIMode::AiAgent, "🤖 AI Agent").clicked() {
                        self.current_mode = UIMode::AiAgent;
                    }
                    ui.label("auto (claude-3.5-sonnet) ⚙");
                });
            });
        });
    }
    
    fn render_action_card(&mut self, ui: &mut egui::Ui, icon: &str, title: &str, description: &str) -> bool {
        let mut clicked = false;
        
        ui.allocate_ui_with_layout([180.0, 120.0].into(), egui::Layout::top_down(egui::Align::Center), |ui| {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_response(rect.size(), egui::Sense::click());
            
            if response.hovered() {
                ui.painter().rect_filled(
                    rect,
                    egui::Rounding::same(8.0),
                    egui::Color32::from_rgb(40, 40, 45)
                );
            } else {
                ui.painter().rect_filled(
                    rect,
                    egui::Rounding::same(8.0),
                    egui::Color32::from_rgb(30, 30, 35)
                );
            }
            
            ui.painter().rect_stroke(
                rect,
                egui::Rounding::same(8.0),
                egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 65))
            );
            
            ui.vertical_centered(|ui| {
                ui.add_space(15.0);
                ui.label(egui::RichText::new(icon).size(24.0));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(title).strong());
                ui.add_space(5.0);
                ui.label(egui::RichText::new(description).small().color(egui::Color32::GRAY));
            });
            
            if response.clicked() {
                clicked = true;
            }
        });
        
        clicked
    }
    
    fn render_terminal_mode(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_terminal(ui);
        });
        
        // Bottom panel for mode switching
        egui::TopBottomPanel::bottom("mode_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.current_mode == UIMode::Welcome, "🏠 Welcome").clicked() {
                    self.current_mode = UIMode::Welcome;
                }
                if ui.selectable_label(self.current_mode == UIMode::Terminal, "🖥 Terminal").clicked() {
                    self.current_mode = UIMode::Terminal;
                }
                if ui.selectable_label(self.current_mode == UIMode::AiAgent, "🤖 AI Agent").clicked() {
                    self.current_mode = UIMode::AiAgent;
                }
            });
        });
    }
    
    fn render_ai_mode(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_ai_panel(ui);
        });
        
        // Bottom panel for mode switching
        egui::TopBottomPanel::bottom("mode_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.current_mode == UIMode::Welcome, "🏠 Welcome").clicked() {
                    self.current_mode = UIMode::Welcome;
                }
                if ui.selectable_label(self.current_mode == UIMode::Terminal, "🖥 Terminal").clicked() {
                    self.current_mode = UIMode::Terminal;
                }
                if ui.selectable_label(self.current_mode == UIMode::AiAgent, "🤖 AI Agent").clicked() {
                    self.current_mode = UIMode::AiAgent;
                }
            });
        });
    }
}

impl eframe::App for AnTraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for AI responses and update the UI accordingly
        while let Ok(ai_response) = self.response_receiver.try_recv() {
            // Find the last AI message (which should be the "Thinking..." placeholder)
            if let Some((role, message)) = self.ai_messages.last_mut() {
                if role == "AI" && message.contains("🤔 Thinking...") {
                    *message = ai_response.content;
                }
            }
        }

        // Dark theme similar to Warp
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.window_fill = egui::Color32::from_rgb(16, 16, 20);
        style.visuals.panel_fill = egui::Color32::from_rgb(16, 16, 20);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(12, 12, 15);
        style.visuals.faint_bg_color = egui::Color32::from_rgb(20, 20, 24);
        ctx.set_style(style);

        match self.current_mode {
            UIMode::Welcome => self.render_welcome_screen(ctx),
            UIMode::Terminal => self.render_terminal_mode(ctx),
            UIMode::AiAgent => self.render_ai_mode(ctx),
        }
    }
}
