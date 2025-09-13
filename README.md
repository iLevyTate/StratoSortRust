# StratoSort Backend

> Rust backend for AI-powered file organization with privacy-first design

A high-performance Rust backend providing AI-powered file organization capabilities. Built for privacy and security, all AI processing happens locally on your machine.

## 🚀 Features

- **🤖 Local AI Analysis** - Uses Ollama for completely private file analysis
- **📁 Smart Organization** - Intelligent file categorization and sorting
- **🔍 Semantic Search** - Find files by content, not just names
- **🎯 Smart Folders** - Dynamic folders based on AI-powered rules
- **⚡ Fast Performance** - Built with high-performance Rust backend
- **🔒 Privacy First** - All AI processing happens locally on your machine
- **🌐 Cross Platform** - Windows, macOS, and Linux support
- **💾 SQLite Database** - Efficient local data storage
- **🔄 Undo/Redo System** - Full operation history with rollback capability

## 📦 Installation

### As a Rust Crate
Add to your `Cargo.toml`:
```toml
[dependencies]
stratosort = { git = "https://github.com/yourusername/StratoSort", path = "src-tauri" }
```

### System Requirements
- **Rust**: 1.75+
- **RAM**: 2GB minimum, 4GB recommended for AI processing
- **Storage**: 500MB free space
- **OS**: Windows 10+, macOS 10.15+, Linux (Ubuntu 20.04+)

## 🤖 AI Setup

StratoSort uses [Ollama](https://ollama.ai/) for local AI processing:

1. **Install Ollama** from https://ollama.ai/
2. **Download a model**: `ollama pull llama3.2:3b`
3. **Start StratoSort** - it will automatically detect Ollama

## 🛠️ Development

### Prerequisites
- [Rust](https://rustup.rs/) 1.75+
- [Ollama](https://ollama.ai/) for AI features (optional)

### Setup
```bash
git clone https://github.com/yourusername/StratoSort.git
cd StratoSort/src-tauri
cargo build
```

### Development Workflow
```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Build release
cargo build --release

# Run linting
cargo clippy

# Format code
cargo fmt
```

### Project Structure
```
src-tauri/
├── src/
│   ├── ai/          # AI processing modules
│   ├── commands/    # API command handlers
│   ├── core/        # Core business logic
│   ├── services/    # Background services
│   ├── storage/     # Database and persistence
│   └── utils/       # Utility functions
├── Cargo.toml       # Rust dependencies
└── tauri.conf.json  # Tauri configuration
```

## 🔒 Security

StratoSort has been thoroughly security audited and includes:

- ✅ **Path traversal protection** with comprehensive validation
- ✅ **Command injection blocking** in system operations
- ✅ **Input sanitization** at multiple layers
- ✅ **Secure database** path resolution
- ✅ **File access validation** with permission checks
- ✅ **Memory safety** guaranteed by Rust
- ✅ **SQL injection prevention** with prepared statements

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Run the test suite
6. Submit a pull request

## 📞 Support

- 🐛 **Bug Reports**: [GitHub Issues](https://github.com/yourusername/StratoSort/issues)
- 💡 **Feature Requests**: [GitHub Discussions](https://github.com/yourusername/StratoSort/discussions)
- 📚 **Documentation**: [Wiki](https://github.com/yourusername/StratoSort/wiki)

## 🎯 Future Goals

- **🔗 REST API** - HTTP API for remote integrations
- **📊 Advanced Analytics** - File organization insights and statistics
- **🔧 Plugin System** - Extensible architecture for custom file handlers
- **📈 Performance Optimization** - Further optimization for large file sets
- **🤖 Enhanced AI Features** - More sophisticated file analysis models
- **🔄 Streaming Processing** - Real-time file organization as files are added
- **📚 Multi-language Support** - Additional AI model language support

## 🙏 Acknowledgments

- [Rust](https://www.rust-lang.org/) - For memory safety and performance
- [Ollama](https://ollama.ai/) - For local AI model execution
- [SQLite](https://www.sqlite.org/) - For reliable embedded database
- [Tokio](https://tokio.rs/) - For async runtime excellence
- [Tauri](https://tauri.app/) - For the original cross-platform framework

---

**StratoSort Backend** - High-performance Rust backend for AI-powered file organization, built with privacy and security in mind.