use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::SystemTime;
use tokio::sync::mpsc as tokio_mpsc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub children: Option<Vec<FileNode>>,
    pub is_expanded: bool,
    pub is_git_ignored: bool,
    pub file_type: FileType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    Directory,
    SourceCode(String), // language
    Config,
    Documentation,
    Image,
    Archive,
    Binary,
    Unknown,
}

impl FileNode {
    pub fn new(path: PathBuf) -> Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let is_directory = metadata.is_dir();
        let size = if is_directory { None } else { Some(metadata.len()) };
        let modified = metadata.modified().ok();
        let file_type = determine_file_type(&path, is_directory);

        Ok(Self {
            name,
            path,
            is_directory,
            size,
            modified,
            children: if is_directory { Some(Vec::new()) } else { None },
            is_expanded: false,
            is_git_ignored: false,
            file_type,
        })
    }

    pub fn toggle_expanded(&mut self) {
        if self.is_directory {
            self.is_expanded = !self.is_expanded;
        }
    }

    pub fn formatted_size(&self) -> String {
        match self.size {
            Some(size) => format_file_size(size),
            None => "—".to_string(),
        }
    }

    pub fn formatted_modified(&self) -> String {
        match self.modified {
            Some(time) => {
                match time.duration_since(SystemTime::UNIX_EPOCH) {
                    Ok(duration) => {
                        let dt = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0);
                        dt.map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Unknown".to_string())
                    }
                    Err(_) => "Unknown".to_string(),
                }
            }
            None => "Unknown".to_string(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match &self.file_type {
            FileType::Directory => if self.is_expanded { "📂" } else { "📁" },
            FileType::SourceCode(lang) => match lang.as_str() {
                "rust" => "🦀",
                "python" => "🐍",
                "javascript" | "typescript" => "📜",
                "go" => "🐹",
                "java" => "☕",
                "c" | "cpp" => "⚙️",
                _ => "📝",
            },
            FileType::Config => "⚙️",
            FileType::Documentation => "📖",
            FileType::Image => "🖼️",
            FileType::Archive => "📦",
            FileType::Binary => "⚫",
            FileType::Unknown => "📄",
        }
    }
}

fn determine_file_type(path: &Path, is_directory: bool) -> FileType {
    if is_directory {
        return FileType::Directory;
    }

    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let filename = path.file_name().and_then(|name| name.to_str()).unwrap_or("");

    // Configuration files
    if matches!(filename, "Cargo.toml" | "package.json" | "pom.xml" | "build.gradle" | 
                        "Makefile" | "CMakeLists.txt" | ".gitignore" | ".dockerignore" |
                        "Dockerfile" | "docker-compose.yml" | "requirements.txt" | "go.mod") {
        return FileType::Config;
    }

    // Documentation
    if matches!(extension, "md" | "rst" | "txt" | "doc" | "docx" | "pdf") {
        return FileType::Documentation;
    }

    // Images
    if matches!(extension, "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp") {
        return FileType::Image;
    }

    // Archives
    if matches!(extension, "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar") {
        return FileType::Archive;
    }

    // Source code
    match extension {
        "rs" => FileType::SourceCode("rust".to_string()),
        "py" => FileType::SourceCode("python".to_string()),
        "js" => FileType::SourceCode("javascript".to_string()),
        "ts" => FileType::SourceCode("typescript".to_string()),
        "go" => FileType::SourceCode("go".to_string()),
        "java" => FileType::SourceCode("java".to_string()),
        "c" => FileType::SourceCode("c".to_string()),
        "cpp" | "cc" | "cxx" => FileType::SourceCode("cpp".to_string()),
        "h" | "hpp" => FileType::SourceCode("c".to_string()),
        "php" => FileType::SourceCode("php".to_string()),
        "rb" => FileType::SourceCode("ruby".to_string()),
        "sh" => FileType::SourceCode("shell".to_string()),
        "html" => FileType::SourceCode("html".to_string()),
        "css" => FileType::SourceCode("css".to_string()),
        "json" => FileType::SourceCode("json".to_string()),
        "xml" => FileType::SourceCode("xml".to_string()),
        "yaml" | "yml" => FileType::SourceCode("yaml".to_string()),
        "toml" => FileType::SourceCode("toml".to_string()),
        _ => {
            // Check if it's a binary file
            if is_likely_binary(path) {
                FileType::Binary
            } else {
                FileType::Unknown
            }
        }
    }
}

fn is_likely_binary(path: &Path) -> bool {
    // Simple heuristic: check if file has executable permissions or common binary extensions
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    matches!(extension, "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "obj")
}

fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size_f = size as f64;
    let mut unit_index = 0;

    while size_f >= 1024.0 && unit_index < UNITS.len() - 1 {
        size_f /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} B", size)
    } else {
        format!("{:.1} {}", size_f, UNITS[unit_index])
    }
}

#[derive(Debug, Clone)]
pub enum FileSystemEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

pub struct FileExplorer {
    root_path: PathBuf,
    root_node: Option<FileNode>,
    watcher: Option<RecommendedWatcher>,
    event_receiver: Option<std::sync::mpsc::Receiver<notify::Result<Event>>>,
    gitignore_patterns: Vec<String>,
    show_hidden_files: bool,
    max_depth: Option<usize>,
}

impl FileExplorer {
    pub fn new(root_path: PathBuf) -> Result<Self> {
        let gitignore_patterns = load_gitignore_patterns(&root_path);

        Ok(Self {
            root_path,
            root_node: None,
            watcher: None,
            event_receiver: None,
            gitignore_patterns,
            show_hidden_files: false,
            max_depth: Some(10), // Prevent infinite recursion
        })
    }

    pub fn load_tree(&mut self) -> Result<()> {
        self.root_node = Some(self.build_tree(&self.root_path.clone(), 0)?);
        Ok(())
    }

    fn build_tree(&self, path: &Path, depth: usize) -> Result<FileNode> {
        let mut node = FileNode::new(path.to_path_buf())?;

        // Check depth limit
        if let Some(max_depth) = self.max_depth {
            if depth >= max_depth {
                return Ok(node);
            }
        }

        // Check if path should be ignored
        if self.should_ignore_path(path) {
            node.is_git_ignored = true;
        }

        if node.is_directory && !node.is_git_ignored {
            let mut children = Vec::new();

            match std::fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let entry_path = entry.path();
                            
                            // Skip hidden files if not showing them
                            if !self.show_hidden_files && self.is_hidden_file(&entry_path) {
                                continue;
                            }

                            match self.build_tree(&entry_path, depth + 1) {
                                Ok(child_node) => children.push(child_node),
                                Err(e) => {
                                    log::warn!("Failed to build tree for {:?}: {}", entry_path, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to read directory {:?}: {}", path, e);
                }
            }

            // Sort children: directories first, then files, both alphabetically
            children.sort_by(|a, b| {
                match (a.is_directory, b.is_directory) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            });

            node.children = Some(children);
        }

        Ok(node)
    }

    fn should_ignore_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        
        // Check gitignore patterns
        for pattern in &self.gitignore_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }

        // Common ignore patterns
        let ignore_patterns = [
            ".git", "node_modules", "target", "build", "dist",
            "__pycache__", ".venv", "venv", ".idea", ".vscode",
        ];

        ignore_patterns.iter().any(|pattern| path_str.contains(pattern))
    }

    fn is_hidden_file(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
    }

    pub fn start_watching(&mut self) -> Result<tokio_mpsc::UnboundedReceiver<FileSystemEvent>> {
        let (tx, rx) = mpsc::channel();
        let (tokio_tx, tokio_rx) = tokio_mpsc::unbounded_channel();

        let mut watcher = notify::recommended_watcher(tx)?;
        watcher.watch(&self.root_path, RecursiveMode::Recursive)?;

        self.watcher = Some(watcher);
        self.event_receiver = Some(rx);

        // Spawn a task to convert notify events to our events
        let event_receiver = self.event_receiver.take().unwrap();
        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv() {
                match event {
                    Ok(notify_event) => {
                        let fs_events = convert_notify_event(notify_event);
                        for fs_event in fs_events {
                            if let Err(_) = tokio_tx.send(fs_event) {
                                break; // Receiver dropped
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File watcher error: {}", e);
                    }
                }
            }
        });

        Ok(tokio_rx)
    }

    pub fn get_root_node(&self) -> Option<&FileNode> {
        self.root_node.as_ref()
    }

    pub fn get_root_node_mut(&mut self) -> Option<&mut FileNode> {
        self.root_node.as_mut()
    }

    pub fn find_node_by_path(&self, path: &Path) -> Option<&FileNode> {
        self.root_node.as_ref().and_then(|root| find_node_recursive(root, path))
    }

    pub fn expand_path(&mut self, path: &Path) -> Result<()> {
        if let Some(root) = self.root_node.as_mut() {
            expand_path_recursive(root, path);
        }
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.load_tree()
    }

    pub fn toggle_hidden_files(&mut self) {
        self.show_hidden_files = !self.show_hidden_files;
    }

    pub fn set_max_depth(&mut self, depth: Option<usize>) {
        self.max_depth = depth;
    }

    pub fn get_file_count(&self) -> usize {
        self.root_node.as_ref().map(count_files).unwrap_or(0)
    }

    pub fn get_directory_count(&self) -> usize {
        self.root_node.as_ref().map(count_directories).unwrap_or(0)
    }

    pub fn search_files(&self, query: &str) -> Vec<&FileNode> {
        let mut results = Vec::new();
        if let Some(root) = &self.root_node {
            search_files_recursive(root, query, &mut results);
        }
        results
    }
}

fn load_gitignore_patterns(root_path: &Path) -> Vec<String> {
    let gitignore_path = root_path.join(".gitignore");
    if let Ok(content) = std::fs::read_to_string(gitignore_path) {
        content
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .map(|line| line.trim().to_string())
            .collect()
    } else {
        Vec::new()
    }
}

fn convert_notify_event(event: Event) -> Vec<FileSystemEvent> {
    let mut fs_events = Vec::new();

    match event.kind {
        notify::EventKind::Create(_) => {
            for path in event.paths {
                fs_events.push(FileSystemEvent::Created(path));
            }
        }
        notify::EventKind::Modify(_) => {
            for path in event.paths {
                fs_events.push(FileSystemEvent::Modified(path));
            }
        }
        notify::EventKind::Remove(_) => {
            for path in event.paths {
                fs_events.push(FileSystemEvent::Deleted(path));
            }
        }
        _ => {} // Ignore other event types for now
    }

    fs_events
}

fn find_node_recursive<'a>(node: &'a FileNode, target_path: &Path) -> Option<&'a FileNode> {
    if node.path == target_path {
        return Some(node);
    }

    if let Some(children) = &node.children {
        for child in children {
            if let Some(found) = find_node_recursive(child, target_path) {
                return Some(found);
            }
        }
    }

    None
}

fn expand_path_recursive(node: &mut FileNode, target_path: &Path) {
    if target_path.starts_with(&node.path) {
        node.is_expanded = true;
        
        if let Some(children) = &mut node.children {
            for child in children {
                expand_path_recursive(child, target_path);
            }
        }
    }
}

fn count_files(node: &FileNode) -> usize {
    let mut count = if !node.is_directory { 1 } else { 0 };
    
    if let Some(children) = &node.children {
        for child in children {
            count += count_files(child);
        }
    }
    
    count
}

fn count_directories(node: &FileNode) -> usize {
    let mut count = if node.is_directory { 1 } else { 0 };
    
    if let Some(children) = &node.children {
        for child in children {
            count += count_directories(child);
        }
    }
    
    count
}

fn search_files_recursive<'a>(node: &'a FileNode, query: &str, results: &mut Vec<&'a FileNode>) {
    if node.name.to_lowercase().contains(&query.to_lowercase()) {
        results.push(node);
    }
    
    if let Some(children) = &node.children {
        for child in children {
            search_files_recursive(child, query, results);
        }
    }
}
