use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub working_directory: String,
    pub exit_code: Option<i32>,
    pub execution_time: Option<u64>, // milliseconds
}

impl HistoryEntry {
    pub fn new(command: String, working_directory: String) -> Self {
        Self {
            command,
            timestamp: Utc::now(),
            working_directory,
            exit_code: None,
            execution_time: None,
        }
    }

    pub fn set_result(&mut self, exit_code: i32, execution_time: u64) {
        self.exit_code = Some(exit_code);
        self.execution_time = Some(execution_time);
    }

    pub fn is_success(&self) -> bool {
        matches!(self.exit_code, Some(0))
    }

    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

pub struct CommandHistory {
    entries: VecDeque<HistoryEntry>,
    max_entries: usize,
    current_index: Option<usize>,
}

impl CommandHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            current_index: None,
        }
    }

    pub fn add_entry(&mut self, entry: HistoryEntry) {
        // Don't add duplicate consecutive entries
        if let Some(last) = self.entries.back() {
            if last.command == entry.command {
                return;
            }
        }

        self.entries.push_back(entry);
        
        // Maintain max size
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        // Reset current index
        self.current_index = None;
    }

    pub fn get_previous(&mut self) -> Option<&HistoryEntry> {
        if self.entries.is_empty() {
            return None;
        }

        match self.current_index {
            None => {
                self.current_index = Some(self.entries.len() - 1);
                self.entries.get(self.entries.len() - 1)
            }
            Some(index) => {
                if index > 0 {
                    self.current_index = Some(index - 1);
                    self.entries.get(index - 1)
                } else {
                    self.entries.get(index)
                }
            }
        }
    }

    pub fn get_next(&mut self) -> Option<&HistoryEntry> {
        match self.current_index {
            None => None,
            Some(index) => {
                if index < self.entries.len() - 1 {
                    self.current_index = Some(index + 1);
                    self.entries.get(index + 1)
                } else {
                    self.current_index = None;
                    None
                }
            }
        }
    }

    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.command.contains(query))
            .collect()
    }

    pub fn search_fuzzy(&self, query: &str) -> Vec<(&HistoryEntry, f64)> {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;

        let matcher = SkimMatcherV2::default();
        let mut matches: Vec<_> = self.entries
            .iter()
            .filter_map(|entry| {
                matcher.fuzzy_match(&entry.command, query)
                    .map(|score| (entry, score as f64))
            })
            .collect();

        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches
    }

    pub fn get_all_entries(&self) -> &VecDeque<HistoryEntry> {
        &self.entries
    }

    pub fn get_recent(&self, count: usize) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .rev()
            .take(count)
            .collect()
    }

    pub fn get_successful_commands(&self) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_success())
            .collect()
    }

    pub fn get_failed_commands(&self) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|entry| !entry.is_success())
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn export_to_file(&self, path: &str) -> Result<()> {
        let content = self.entries
            .iter()
            .map(|entry| {
                format!(
                    "{} {} ({}){}\n",
                    entry.formatted_timestamp(),
                    entry.command,
                    entry.working_directory,
                    match entry.exit_code {
                        Some(0) => " ✓".to_string(),
                        Some(code) => format!(" ✗({})", code),
                        None => "".to_string(),
                    }
                )
            })
            .collect::<String>();

        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn import_from_shell_history(&mut self, shell: &str) -> Result<usize> {
        let history_file = match shell {
            "bash" => {
                dirs::home_dir()
                    .map(|home| home.join(".bash_history"))
                    .and_then(|path| if path.exists() { Some(path) } else { None })
            }
            "zsh" => {
                dirs::home_dir()
                    .map(|home| home.join(".zsh_history"))
                    .and_then(|path| if path.exists() { Some(path) } else { None })
            }
            "fish" => {
                dirs::config_dir()
                    .map(|config| config.join("fish").join("fish_history"))
                    .and_then(|path| if path.exists() { Some(path) } else { None })
            }
            "pwsh" | "powershell" => {
                // PowerShell history is more complex, skip for now
                return Ok(0);
            }
            _ => None,
        };

        if let Some(history_path) = history_file {
            let content = std::fs::read_to_string(history_path)?;
            let mut imported = 0;

            for line in content.lines() {
                if !line.trim().is_empty() && !line.starts_with('#') {
                    let entry = HistoryEntry::new(
                        line.trim().to_string(),
                        std::env::current_dir()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    );
                    self.add_entry(entry);
                    imported += 1;
                }
            }

            Ok(imported)
        } else {
            Ok(0)
        }
    }
}
