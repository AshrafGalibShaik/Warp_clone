# Warp_clone 🚀

**Next-Generation AI-Powered Terminal Application**

ANTRAFT is an intelligent, developer-friendly terminal application inspired by Warp, built with modern Rust technologies and integrated AI capabilities. It combines the power of a GPU-accelerated terminal with AI assistance, security scanning, and intelligent code completion.

## ✨ Features

### 🖥️ Modern Terminal Experience
- **GPU-accelerated rendering** with WGPU for smooth performance
- **Block-based input/output** preserving command context like Warp
- **Tab and split-pane support** for multiple terminal sessions
- **Advanced PTY management** with proper terminal emulation

### 🤖 AI Assistant Integration
- **Gemini 2.0 Flash integration** for intelligent command assistance
- **Command explanation** - Ask "What does this command do?"
- **Error fixing** - Get AI-powered solutions for command errors
- **Code review** - Automated code quality analysis
- **Command generation** - Describe what you want, get the command

### 🔍 Security & Vulnerability Detection
- **Multi-tool scanning** with Bandit, Semgrep, and OSV-Scanner integration
- **Real-time vulnerability detection** on written code
- **AI-powered security analysis** with fix suggestions
- **Comprehensive security reports** with risk scoring

### 📁 Intelligent File Management
- **Integrated file explorer** with project navigation
- **Git-aware file handling** with .gitignore support
- **Real-time file watching** with automatic updates
- **File type detection** with appropriate icons and handling

### ⚡ Smart Developer Tools
- **Fuzzy autocomplete** with command history integration
- **Syntax highlighting** powered by Tree-sitter
- **Git integration** with branch and status awareness
- **Multi-shell support** (bash, zsh, fish, PowerShell)

## 🛠️ Technology Stack

- **Frontend**: Rust + egui + WGPU for GPU acceleration
- **Terminal Core**: tokio + portable-pty for async shell execution
- **AI Integration**: Gemini Pro 2.0 API with custom agents
- **Security Tools**: Bandit, Semgrep, OSV-Scanner integration
- **Parsing**: Tree-sitter for syntax highlighting and analysis

## 📦 Installation

### Prerequisites

1. **Rust** (latest stable version)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Security Tools** (optional but recommended)
   ```bash
   # Install Bandit for Python security scanning
   pip install bandit
   
   # Install Semgrep for multi-language security scanning
   pip install semgrep
   
   # Install OSV-Scanner for dependency vulnerability scanning
   go install github.com/google/osv-scanner/cmd/osv-scanner@v1
   ```

3. **Gemini API Key**
   ```bash
   export GEMINI_API_KEY="your_api_key_here"
   ```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/antraft/antraft.git
cd antraft

# Build the application
cargo build --release

# Run ANTRAFT
./target/release/antraft
```

### Quick Start

```bash
# Start with debug logging
./target/release/antraft --debug

# Start in a specific directory
./target/release/antraft --directory /path/to/project

# Use custom configuration
./target/release/antraft --config /path/to/config.toml
```

## ⚙️ Configuration

ANTRAFT uses a TOML configuration file located at:
- **Linux/macOS**: `~/.config/antraft/config.toml`
- **Windows**: `%APPDATA%/antraft/config.toml`

### Sample Configuration

```toml
[ai]
api_key = "your_gemini_api_key"
model = "gemini-pro"
max_tokens = 2048
temperature = 0.7
system_prompt = "You are an AI assistant integrated into ANTRAFT..."

[security]
enable_bandit = true
enable_semgrep = true
enable_osv = true
scan_timeout_seconds = 300
max_file_size_mb = 10
excluded_paths = ["node_modules", ".git", "target"]

[terminal]
shell = "bash"  # or "zsh", "fish", "pwsh"
font_size = 14.0
theme = "dark"
max_history = 1000
enable_vi_mode = false
```

## 🎯 Usage Examples

### Basic Terminal Operations
```bash
# The terminal works like any standard terminal
ls -la
cd my-project
git status
```

### AI Command Assistance
- Type a command and ask: **"What does this do?"**
- Get error explanations: **"Fix this error: permission denied"**
- Generate commands: **"Create a git branch called feature-x"**

### Security Scanning
```bash
# Built-in security scan command
scan-project

# Quick dependency scan
scan-project --type dependencies

# Full security audit
scan-project --type full
```

### File Explorer Integration
- **Browse files** in the integrated sidebar
- **Right-click** to open terminal in file's directory
- **Double-click** to run files or open in editor

## 🔧 Development

### Project Structure
```
antraft/
├── src/
│   ├── main.rs              # Application entry point
│   ├── terminal/            # Terminal engine and PTY management
│   ├── ai/                  # AI agent and Gemini integration
│   ├── security/            # Security scanning modules
│   ├── file_explorer/       # File system navigation
│   ├── autocomplete/        # Command completion engine
│   └── ui/                  # User interface components
├── tests/                   # Test suites
├── docs/                    # Documentation
└── assets/                  # Static assets
```

### Building for Development
```bash
# Run in development mode
cargo run

# Run tests
cargo test

# Check code formatting
cargo fmt --check

# Run clippy lints
cargo clippy -- -D warnings
```

### Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 🚦 Roadmap

- [ ] **Advanced Terminal Features**
  - [ ] Terminal multiplexer integration (tmux/screen)
  - [ ] Custom themes and color schemes
  - [ ] Terminal recording and playback

- [ ] **Enhanced AI Capabilities**
  - [ ] GPT-4 integration option
  - [ ] Custom AI model support
  - [ ] Offline AI capabilities

- [ ] **Extended Security Features**
  - [ ] Custom security rule definitions
  - [ ] Integration with more security tools
  - [ ] Continuous security monitoring

- [ ] **Platform Support**
  - [ ] macOS native app bundle
  - [ ] Windows installer
  - [ ] Linux AppImage/Flatpak

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by [Warp Terminal](https://www.warp.dev/) for the modern terminal experience
- Built with [egui](https://github.com/emilk/egui) for the user interface
- Powered by [Google Gemini](https://deepmind.google/technologies/gemini/) for AI capabilities
- Security tools: [Bandit](https://github.com/PyCQA/bandit), [Semgrep](https://github.com/returntocorp/semgrep), [OSV-Scanner](https://github.com/google/osv-scanner)

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/antraft/antraft/issues)
- **Discussions**: [GitHub Discussions](https://github.com/antraft/antraft/discussions)
- **Documentation**: [docs/](docs/)

---
