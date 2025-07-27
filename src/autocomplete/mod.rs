use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tree_sitter::Parser;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutocompleteItem {
    pub text: String,
    pub description: String,
    pub category: String,
    pub priority: i32,
    pub snippet: Option<String>,
    pub insert_text: String,
}

impl AutocompleteItem {
    pub fn new(text: String, description: String, category: String) -> Self {
        Self {
            insert_text: text.clone(),
            text,
            description,
            category,
            priority: 0,
            snippet: None,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_snippet(mut self, snippet: String) -> Self {
        self.snippet = Some(snippet.clone());
        self.insert_text = snippet;
        self
    }
}

pub struct AutocompleteEngine {
    matcher: SkimMatcherV2,
    command_providers: Vec<Box<dyn AutocompleteProvider>>,
    user_history: Vec<String>,
    max_suggestions: usize,
}

impl AutocompleteEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            matcher: SkimMatcherV2::default(),
            command_providers: Vec::new(),
            user_history: Vec::new(),
            max_suggestions: 10,
        };

        // Add built-in providers
        engine.add_provider(Box::new(BuiltinCommandProvider::new()));
        engine.add_provider(Box::new(GitCommandProvider::new()));
        engine.add_provider(Box::new(FileSystemProvider::new()));
        engine.add_provider(Box::new(HistoryProvider::new()));

        engine
    }

    pub fn add_provider(&mut self, provider: Box<dyn AutocompleteProvider>) {
        self.command_providers.push(provider);
    }

    pub fn get_suggestions(
        &self,
        input: &str,
        context: &AutocompleteContext,
    ) -> Vec<AutocompleteItem> {
        let mut all_suggestions = Vec::new();

        // Get suggestions from all providers
        for provider in &self.command_providers {
            let mut provider_suggestions = provider.get_suggestions(input, context);
            all_suggestions.append(&mut provider_suggestions);
        }

        // Score and sort suggestions
        let mut scored_suggestions: Vec<_> = all_suggestions
            .into_iter()
            .filter_map(|item| {
                self.matcher
                    .fuzzy_match(&item.text, input)
                    .map(|score| (item.clone(), score + item.priority as i64))
            })
            .collect();

        scored_suggestions.sort_by(|a, b| b.1.cmp(&a.1));

        // Return top suggestions
        scored_suggestions
            .into_iter()
            .take(self.max_suggestions)
            .map(|(item, _)| item)
            .collect()
    }

    pub fn add_to_history(&mut self, command: String) {
        if !command.trim().is_empty() && !self.user_history.contains(&command) {
            self.user_history.push(command);
            if self.user_history.len() > 1000 {
                self.user_history.remove(0);
            }
        }
    }

    pub fn get_history(&self) -> &[String] {
        &self.user_history
    }

    pub fn set_max_suggestions(&mut self, max: usize) {
        self.max_suggestions = max;
    }
}

#[derive(Debug, Clone)]
pub struct AutocompleteContext {
    pub current_directory: String,
    pub shell: String,
    pub recent_commands: Vec<String>,
    pub git_repository: bool,
    pub file_extensions: Vec<String>,
}

impl AutocompleteContext {
    pub fn new(current_directory: String, shell: String) -> Self {
        Self {
            current_directory,
            shell,
            recent_commands: Vec::new(),
            git_repository: false,
            file_extensions: Vec::new(),
        }
    }

    pub fn with_git_repository(mut self, is_git_repo: bool) -> Self {
        self.git_repository = is_git_repo;
        self
    }

    pub fn with_recent_commands(mut self, commands: Vec<String>) -> Self {
        self.recent_commands = commands;
        self
    }

    pub fn with_file_extensions(mut self, extensions: Vec<String>) -> Self {
        self.file_extensions = extensions;
        self
    }
}

pub trait AutocompleteProvider {
    fn get_suggestions(&self, input: &str, context: &AutocompleteContext) -> Vec<AutocompleteItem>;
    fn name(&self) -> &str;
}

pub struct BuiltinCommandProvider {
    commands: HashMap<String, AutocompleteItem>,
}

impl BuiltinCommandProvider {
    pub fn new() -> Self {
        let mut commands = HashMap::new();

        // Common Unix/Linux commands
        let builtin_commands = [
            ("ls", "List directory contents", "filesystem"),
            ("cd", "Change directory", "navigation"),
            ("pwd", "Print working directory", "navigation"),
            ("mkdir", "Create directory", "filesystem"),
            ("rmdir", "Remove directory", "filesystem"),
            ("rm", "Remove files and directories", "filesystem"),
            ("cp", "Copy files and directories", "filesystem"),
            ("mv", "Move/rename files and directories", "filesystem"),
            ("find", "Search for files and directories", "search"),
            ("grep", "Search text patterns", "search"),
            ("cat", "Display file contents", "filesystem"),
            ("less", "View file contents page by page", "filesystem"),
            ("head", "Display first lines of file", "filesystem"),
            ("tail", "Display last lines of file", "filesystem"),
            ("chmod", "Change file permissions", "filesystem"),
            ("chown", "Change file ownership", "filesystem"),
            ("ps", "List running processes", "system"),
            ("kill", "Terminate processes", "system"),
            ("top", "Display running processes", "system"),
            ("htop", "Interactive process viewer", "system"),
            ("df", "Display filesystem disk space", "system"),
            ("du", "Display directory disk usage", "system"),
            ("free", "Display memory usage", "system"),
            ("uname", "Display system information", "system"),
            ("whoami", "Display current username", "system"),
            ("which", "Locate command", "system"),
            ("history", "Command history", "history"),
            ("clear", "Clear terminal screen", "terminal"),
            ("exit", "Exit terminal", "terminal"),
        ];

        for (cmd, desc, category) in builtin_commands {
            commands.insert(
                cmd.to_string(),
                AutocompleteItem::new(cmd.to_string(), desc.to_string(), category.to_string())
                    .with_priority(10),
            );
        }

        // Windows specific commands
        if cfg!(windows) {
            let windows_commands = [
                ("dir", "List directory contents", "filesystem"),
                ("type", "Display file contents", "filesystem"),
                ("copy", "Copy files", "filesystem"),
                ("move", "Move files", "filesystem"),
                ("del", "Delete files", "filesystem"),
                ("md", "Create directory", "filesystem"),
                ("rd", "Remove directory", "filesystem"),
                ("cls", "Clear screen", "terminal"),
            ];

            for (cmd, desc, category) in windows_commands {
                commands.insert(
                    cmd.to_string(),
                    AutocompleteItem::new(cmd.to_string(), desc.to_string(), category.to_string())
                        .with_priority(10),
                );
            }
        };

        Self { commands }
    }
}

impl AutocompleteProvider for BuiltinCommandProvider {
    fn get_suggestions(
        &self,
        input: &str,
        _context: &AutocompleteContext,
    ) -> Vec<AutocompleteItem> {
        self.commands
            .values()
            .filter(|item| item.text.starts_with(input))
            .cloned()
            .collect()
    }

    fn name(&self) -> &str {
        "builtin"
    }
}

pub struct GitCommandProvider {
    commands: HashMap<String, AutocompleteItem>,
}

impl GitCommandProvider {
    pub fn new() -> Self {
        let mut commands = HashMap::new();

        let git_commands = [
            ("git add", "Add files to staging area"),
            ("git commit", "Create a new commit"),
            ("git push", "Upload changes to remote repository"),
            ("git pull", "Download and merge changes from remote"),
            ("git status", "Display the state of the working directory"),
            ("git log", "Display commit logs"),
            (
                "git diff",
                "Show changes between commits, commit and working tree, etc",
            ),
            ("git branch", "Manage branches"),
            (
                "git checkout",
                "Switch branches or restore working tree files",
            ),
            (
                "git merge",
                "Join two or more development histories together",
            ),
            ("git clone", "Clone a repository into a new directory"),
            ("git init", "Create an empty Git repository"),
            ("git remote", "Manage set of tracked repositories"),
            (
                "git fetch",
                "Download objects and refs from another repository",
            ),
            ("git reset", "Reset current HEAD to the specified state"),
            (
                "git stash",
                "Stash the changes in a dirty working directory away",
            ),
        ];

        for (cmd, desc) in git_commands {
            commands.insert(
                cmd.to_string(),
                AutocompleteItem::new(cmd.to_string(), desc.to_string(), "git".to_string())
                    .with_priority(15),
            );
        }

        Self { commands }
    }
}

impl AutocompleteProvider for GitCommandProvider {
    fn get_suggestions(&self, input: &str, context: &AutocompleteContext) -> Vec<AutocompleteItem> {
        if !context.git_repository {
            return Vec::new();
        }

        self.commands
            .values()
            .filter(|item| item.text.starts_with(input))
            .cloned()
            .collect()
    }

    fn name(&self) -> &str {
        "git"
    }
}

pub struct FileSystemProvider;

impl FileSystemProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_directory_entries(&self, dir_path: &str) -> Vec<AutocompleteItem> {
        let mut entries = Vec::new();

        if let Ok(dir_entries) = std::fs::read_dir(dir_path) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if !name.is_empty() && !name.starts_with('.') {
                    let is_dir = path.is_dir();
                    let display_name = if is_dir {
                        format!("{}/", name)
                    } else {
                        name.clone()
                    };

                    let category = if is_dir { "directory" } else { "file" };
                    let description = format!("{} ({})", name, category);

                    entries.push(
                        AutocompleteItem::new(display_name, description, category.to_string())
                            .with_priority(if is_dir { 8 } else { 5 }),
                    );
                }
            }
        }

        entries.sort_by(|a, b| a.text.cmp(&b.text));
        entries
    }
}

impl AutocompleteProvider for FileSystemProvider {
    fn get_suggestions(&self, input: &str, context: &AutocompleteContext) -> Vec<AutocompleteItem> {
        // Only provide file/directory completions if input looks like a path
        if input.contains('/') || input.contains('\\') {
            let path_parts: Vec<&str> = input.rsplitn(2, |c| c == '/' || c == '\\').collect();
            if path_parts.len() == 2 {
                let (filename_part, dir_part) = (path_parts[0], path_parts[1]);
                let search_dir = if dir_part.is_empty() {
                    context.current_directory.clone()
                } else {
                    format!("{}/{}", context.current_directory, dir_part)
                };

                return self
                    .get_directory_entries(&search_dir)
                    .into_iter()
                    .filter(|item| item.text.starts_with(filename_part))
                    .collect();
            }
        }

        // For simple filenames, search in current directory
        if !input.is_empty()
            && input
                .chars()
                .all(|c| c == '.' || c == '_' || c == '-' || c.is_alphanumeric())
        {
            return self
                .get_directory_entries(&context.current_directory)
                .into_iter()
                .filter(|item| item.text.starts_with(input))
                .collect();
        }

        Vec::new()
    }

    fn name(&self) -> &str {
        "filesystem"
    }
}

pub struct HistoryProvider;

impl HistoryProvider {
    pub fn new() -> Self {
        Self
    }
}

impl AutocompleteProvider for HistoryProvider {
    fn get_suggestions(&self, input: &str, context: &AutocompleteContext) -> Vec<AutocompleteItem> {
        context
            .recent_commands
            .iter()
            .filter(|cmd| cmd.starts_with(input))
            .enumerate()
            .map(|(i, cmd)| {
                AutocompleteItem::new(
                    cmd.clone(),
                    "From command history".to_string(),
                    "history".to_string(),
                )
                .with_priority(20 - i as i32) // Recent commands get higher priority
            })
            .collect()
    }

    fn name(&self) -> &str {
        "history"
    }
}

pub struct SyntaxHighlighter {
    parsers: HashMap<String, Parser>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let mut highlighter = Self {
            parsers: HashMap::new(),
        };

        // Initialize parsers for supported languages
        // TODO: Revisit tree_sitter_bash integration due to LanguageFn error
        // Temporarily commented out to allow compilation
        /*
        highlighter.parsers.insert(
            "bash".to_string(),
            {
                let mut parser = Parser::new();
                parser.set_language(unsafe { LANGUAGE() }).expect("Failed to set bash language");
                parser
            }
        );
        */

        highlighter
    }

    pub fn highlight(&mut self, text: &str, language: &str) -> Vec<(usize, usize, String)> {
        // Returns (start, end, class) tuples for highlighting
        let mut highlights = Vec::new();

        if let Some(parser) = self.parsers.get_mut(language) {
            if let Some(tree) = parser.parse(text, None) {
                // This is a simplified highlighter - in a real implementation,
                // you'd use tree-sitter queries to extract syntax highlighting information
                let root_node = tree.root_node();
                self.highlight_node(root_node, text.as_bytes(), &mut highlights);
            }
        }

        highlights
    }

    fn highlight_node(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        highlights: &mut Vec<(usize, usize, String)>,
    ) {
        let start = node.start_byte();
        let end = node.end_byte();
        let kind = node.kind();

        // Map node kinds to CSS classes
        let class = match kind {
            "comment" => "comment",
            "string" => "string",
            "number" => "number",
            "identifier" => "identifier",
            "keyword" => "keyword",
            _ => "default",
        };

        if class != "default" {
            highlights.push((start, end, class.to_string()));
        }

        // Recursively highlight child nodes
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.highlight_node(child, source, highlights);
            }
        }
    }
}
