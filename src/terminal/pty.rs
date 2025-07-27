use anyhow::Result;
use log::{debug, error};
use portable_pty::{CommandBuilder, PtyPair, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct PtyManager {
    pty_system: Box<dyn PtySystem>,
}

impl PtyManager {
    pub fn new() -> Result<Self> {
        let pty_system = portable_pty::native_pty_system();
        Ok(Self { pty_system })
    }

    pub fn create_pty(&self, rows: u16, cols: u16, shell: &str) -> Result<PtySession> {
        let pty_pair = self.pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        
        if cfg!(windows) {
            // For Windows PowerShell
            cmd.args(&["-NoLogo", "-NoExit"]);
        } else {
            // For Unix shells
            cmd.args(&["-i"]); // Interactive mode
        }

        let child = pty_pair.slave.spawn_command(cmd)?;
        
        debug!("Created PTY session with PID: {:?}", child.process_id());

        Ok(PtySession {
            pty_pair,
            child: Some(child),
        })
    }
}

pub struct PtySession {
    pub pty_pair: PtyPair,
    pub child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
}

impl PtySession {
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.pty_pair.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        // Use the portable_pty write method
        let mut writer = self.pty_pair.master.take_writer()?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    pub fn read_output(&mut self, buffer: &mut [u8]) -> Result<usize> {
        let mut reader = self.pty_pair.master.try_clone_reader()?;
        let bytes_read = reader.read(buffer)?;
        Ok(bytes_read)
    }

    pub fn is_child_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => false, // Child has exited
                Ok(None) => true,     // Child is still running
                Err(_) => false,      // Error checking status, assume dead
            }
        } else {
            false
        }
    }

    pub fn kill_child(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Err(e) = self.kill_child() {
            error!("Failed to kill PTY child process: {}", e);
        }
    }
}

// VTE (Virtual Terminal Emulator) parser for handling terminal escape sequences
pub struct VteProcessor {
    parser: vte::Parser,
    performer: VtePerformer,
}

impl VteProcessor {
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            performer: VtePerformer::new(),
        }
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) -> Vec<TerminalAction> {
        self.performer.clear_actions();
        
        for byte in bytes {
            self.parser.advance(&mut self.performer, *byte);
        }
        
        self.performer.take_actions()
    }
}

struct VtePerformer {
    actions: Vec<TerminalAction>,
}

impl VtePerformer {
    fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    fn clear_actions(&mut self) {
        self.actions.clear();
    }

    fn take_actions(&mut self) -> Vec<TerminalAction> {
        std::mem::take(&mut self.actions)
    }
}

#[derive(Debug, Clone)]
pub enum TerminalAction {
    Print(char),
    LineFeed,
    CarriageReturn,
    Backspace,
    Tab,
    ClearScreen,
    ClearLine,
    SetCursorPosition { row: usize, col: usize },
    SetForegroundColor { r: u8, g: u8, b: u8 },
    SetBackgroundColor { r: u8, g: u8, b: u8 },
    SetBold(bool),
    SetItalic(bool),
    SetUnderline(bool),
    Reset,
}

impl vte::Perform for VtePerformer {
    fn print(&mut self, c: char) {
        self.actions.push(TerminalAction::Print(c));
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.actions.push(TerminalAction::LineFeed),
            b'\r' => self.actions.push(TerminalAction::CarriageReturn),
            b'\x08' => self.actions.push(TerminalAction::Backspace),
            b'\t' => self.actions.push(TerminalAction::Tab),
            _ => {} // Ignore other control characters for now
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _c: char) {
        // Handle DCS sequences if needed
    }

    fn put(&mut self, _byte: u8) {
        // Handle DCS data
    }

    fn unhook(&mut self) {
        // End of DCS sequence
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {
        // Handle OSC (Operating System Command) sequences
    }

    fn csi_dispatch(&mut self, params: &vte::Params, _intermediates: &[u8], _ignore: bool, c: char) {
        match c {
            'H' | 'f' => {
                // Cursor Position
                let row = params.iter().next().and_then(|p| p[0].try_into().ok()).unwrap_or(1);
                let col = params.iter().nth(1).and_then(|p| p[0].try_into().ok()).unwrap_or(1);
                self.actions.push(TerminalAction::SetCursorPosition { 
                    row: (row as usize).saturating_sub(1), 
                    col: (col as usize).saturating_sub(1) 
                });
            }
            'J' => {
                // Erase Display
                let param = params.iter().next().map(|p| p[0]).unwrap_or(0);
                if param == 2 {
                    self.actions.push(TerminalAction::ClearScreen);
                }
            }
            'K' => {
                // Erase Line
                self.actions.push(TerminalAction::ClearLine);
            }
            'm' => {
                // Select Graphic Rendition (SGR)
                for param in params.iter() {
                    match param[0] {
                        0 => self.actions.push(TerminalAction::Reset),
                        1 => self.actions.push(TerminalAction::SetBold(true)),
                        3 => self.actions.push(TerminalAction::SetItalic(true)),
                        4 => self.actions.push(TerminalAction::SetUnderline(true)),
                        22 => self.actions.push(TerminalAction::SetBold(false)),
                        23 => self.actions.push(TerminalAction::SetItalic(false)),
                        24 => self.actions.push(TerminalAction::SetUnderline(false)),
                        30..=37 => {
                            // Basic foreground colors
                            let colors = [
                                (0, 0, 0),       // Black
                                (128, 0, 0),     // Red
                                (0, 128, 0),     // Green
                                (128, 128, 0),   // Yellow
                                (0, 0, 128),     // Blue
                                (128, 0, 128),   // Magenta
                                (0, 128, 128),   // Cyan
                                (192, 192, 192), // White
                            ];
                            if let Some((r, g, b)) = colors.get((param[0] - 30) as usize) {
                                self.actions.push(TerminalAction::SetForegroundColor { r: *r, g: *g, b: *b });
                            }
                        }
                        40..=47 => {
                            // Basic background colors
                            let colors = [
                                (0, 0, 0),       // Black
                                (128, 0, 0),     // Red
                                (0, 128, 0),     // Green
                                (128, 128, 0),   // Yellow
                                (0, 0, 128),     // Blue
                                (128, 0, 128),   // Magenta
                                (0, 128, 128),   // Cyan
                                (192, 192, 192), // White
                            ];
                            if let Some((r, g, b)) = colors.get((param[0] - 40) as usize) {
                                self.actions.push(TerminalAction::SetBackgroundColor { r: *r, g: *g, b: *b });
                            }
                        }
                        _ => {} // Ignore unknown SGR parameters
                    }
                }
            }
            _ => {} // Ignore other CSI sequences for now
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        // Handle ESC sequences
    }
}
