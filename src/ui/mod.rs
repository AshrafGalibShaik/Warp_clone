use anyhow::Result;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::ai::{AiAgent, AiConfig, AiRequest, AiResponse};
use crate::autocomplete::{AutocompleteEngine, AutocompleteContext};
use crate::file_explorer::FileExplorer;
use crate::terminal::{TerminalEngine, TerminalEvent, TerminalEventSender, TerminalSession};
use crate::security::{SecurityScanner, SecurityConfig, SecurityScanRequest, ScanType};
use log::info;
use crossbeam_channel;
use tokio::sync::mpsc;
use std::collections::VecDeque;

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
    command_input: String,
    command_history: VecDeque<String>,
    terminal_output: Vec<TerminalBlock>,
    show_ai_panel: bool,
    ai_input: String,
    ai_messages: Vec<(String, String)>, // (role, message)
    selected_tab: TabSelection,
    runtime: Arc<tokio::runtime::Runtime>,
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
enum TabSelection {
    Terminal,
    FileExplorer,
    Security,
}

impl AnTraftApp {
    pub async fn new(config: Config) -> Result<Self> {
        let (terminal_event_tx, _terminal_event_rx) = tokio::sync::mpsc::unbounded_channel();

        let terminal_engine = TerminalEngine::new(config.terminal.clone(), terminal_event_tx.clone())?;
        let ai_agent = Arc::new(RwLock::new(AiAgent::new(config.ai.clone())));
        let file_explorer = Arc::new(RwLock::new(FileExplorer::new(std::env::current_dir()?)?));
        let autocomplete_engine = Arc::new(RwLock::new(AutocompleteEngine::new()));
        let security_scanner = Arc::new(SecurityScanner::new(config.security.clone())?);

        let (response_sender, response_receiver) = crossbeam_channel::unbounded();
        
        let runtime = Arc::new(tokio::runtime::Runtime::new()?);

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
            command_input: String::new(),
            command_history: VecDeque::new(),
            terminal_output: Vec::new(),
            show_ai_panel: false,
            ai_input: String::new(),
            ai_messages: Vec::new(),
            selected_tab: TabSelection::Terminal,
            runtime,
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

    pub async fn execute_terminal_command(&self, command: String) -> Result<()> {
        let response_tx = self.response_sender.clone();
        let session = self.terminal_engine.get_active_session().await.unwrap_or(TerminalSession::new());
        
        // Execute command
        self.terminal_engine.execute_command(command.clone()).await?;

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

    pub async fn perform_autocomplete(&self, input: String, context: AutocompleteContext) -> Result<Vec<String>> {
        let engine = self.autocomplete_engine.read().await;
        let suggestions = engine.get_suggestions(&input, &context);
        Ok(suggestions.into_iter().map(|s| s.insert_text).collect())
    }
}

impl eframe::App for AnTraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark theme similar to Warp
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.window_fill = egui::Color32::from_rgb(24, 24, 27);
        style.visuals.panel_fill = egui::Color32::from_rgb(24, 24, 27);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(16, 16, 18);
        style.visuals.faint_bg_color = egui::Color32::from_rgb(30, 30, 34);
        ctx.set_style(style);

        // Top panel with tabs
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🚀 ANTRAFT");
                ui.separator();
                
                ui.selectable_value(&mut self.selected_tab, TabSelection::Terminal, "Terminal");
                ui.selectable_value(&mut self.selected_tab, TabSelection::FileExplorer, "Files");
                ui.selectable_value(&mut self.selected_tab, TabSelection::Security, "Security");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("AI Assistant").clicked() {
                        self.show_ai_panel = !self.show_ai_panel;
                    }
                });
            });
        });

        // AI Assistant Panel (Right side)
        if self.show_ai_panel {
            egui::SidePanel::right("ai_panel")
                .default_width(300.0)
                .show(ctx, |ui| {
                    self.render_ai_panel(ui);
                });
        }

        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.selected_tab {
                TabSelection::Terminal => self.render_terminal(ui, ctx),
                TabSelection::FileExplorer => self.render_file_explorer(ui),
                TabSelection::Security => self.render_security_panel(ui),
            }
        });
    }
    
    fn render_terminal(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let available_height = ui.available_height();
        
        // Terminal output area with blocks (like Warp)
        egui::ScrollArea::vertical()
            .max_height(available_height - 60.0)
            .show(ui, |ui| {
                for (i, block) in self.terminal_output.iter().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("[{}]", i + 1));
                            ui.label(&block.command);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(block.timestamp.format("%H:%M:%S").to_string());
                            });
                        });
                        
                        ui.separator();
                        
                        ui.add_space(4.0);
                        ui.monospace(&block.output);
                        
                        if block.is_running {
                            ui.spinner();
                        }
                    });
                    ui.add_space(8.0);
                }
            });
        
        // Command input area at bottom
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("❯");
            let response = ui.text_edit_singleline(&mut self.command_input)
                .desired_width(ui.available_width() - 100.0);
            
            // Handle Enter key
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.execute_command();
            }
            
            if ui.button("Run").clicked() {
                self.execute_command();
            }
        });
    }
    
    fn render_ai_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("AI Assistant");
        ui.separator();
        
        // Chat messages
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 100.0)
            .show(ui, |ui| {
                for (role, message) in &self.ai_messages {
                    ui.group(|ui| {
                        ui.label(format!("{}: ", role));
                        ui.label(message);
                    });
                    ui.add_space(4.0);
                }
            });
        
        // Input area
        ui.separator();
        ui.horizontal(|ui| {
            let response = ui.text_edit_singleline(&mut self.ai_input)
                .desired_width(ui.available_width() - 60.0);
            
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.send_ai_message();
            }
            
            if ui.button("Send").clicked() {
                self.send_ai_message();
            }
        });
    }
    
    fn render_file_explorer(&mut self, ui: &mut egui::Ui) {
        ui.heading("File Explorer");
        ui.separator();
        
        // TODO: Implement file tree view
        ui.label("File explorer coming soon...");
    }
    
    fn render_security_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Security Scanner");
        ui.separator();
        
        ui.horizontal(|ui| {
            if ui.button("Quick Scan").clicked() {
                self.start_security_scan(ScanType::Quick);
            }
            if ui.button("Full Scan").clicked() {
                self.start_security_scan(ScanType::Full);
            }
        });
        
        ui.separator();
        ui.label("Scan results will appear here...");
    }
    
    fn execute_command(&mut self) {
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
        let runtime = self.runtime.clone();
        let block_id = block.id;
        let mut output_blocks = self.terminal_output.clone();
        
        runtime.spawn(async move {
            // Simulate command execution
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            // Update the block with output
            if let Some(block) = output_blocks.iter_mut().find(|b| b.id == block_id) {
                block.output = format!("Executed: {}", command);
                block.is_running = false;
            }
        });
    }
    
    fn send_ai_message(&mut self) {
        if self.ai_input.is_empty() {
            return;
        }
        
        let message = self.ai_input.clone();
        self.ai_messages.push(("You".to_string(), message.clone()));
        self.ai_input.clear();
        
        // TODO: Send to AI agent
        self.ai_messages.push(("AI".to_string(), "Response coming soon...".to_string()));
    }
    
    fn start_security_scan(&mut self, scan_type: ScanType) {
        info!("Starting {:?} security scan", scan_type);
        // TODO: Implement actual security scan
    }
}
