<p align="center">
  <img src="assets/logo.svg" alt="Stratosort" width="96" height="96">
</p>

<h1 align="center">Stratosort</h1>

<p align="center">
  Local-first AI file organization. Vision, embeddings, and semantic search all running on your machine via <a href="https://ollama.com/">Ollama</a>.
</p>

<p align="center">
  <a href="https://github.com/iLevyTate/StratoSortRust/actions/workflows/ci.yml"><img src="https://github.com/iLevyTate/StratoSortRust/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
  <a href="#"><img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust 1.75+"></a>
  <a href="#"><img src="https://img.shields.io/badge/tauri-v2-yellow.svg" alt="Tauri v2"></a>
</p>

---

> **Project status:** The backend foundation is built and working the AI dispatcher, embeddings, semantic search, watch mode, and storage are all functional and exercised by the test suite. Frontend integration (wiring these features into the Tauri desktop UI) is a work in progress.

## What it does

Drop a folder of mixed files — images, PDFs, Word docs, spreadsheets, notes — into a watched directory. Stratosort routes each one through the right local model:

| File kind | Model | What you get |
|---|---|---|
| Images (PNG/JPG/WEBP) | `llava:7b` | Visual description + detected objects + scene type |
| PDF / DOCX / XLSX / CSV / Markdown | `llama3.2:3b` | Extracted text → summary + tags + category |
| Plain text / source code | `llama3.2:3b` | Summary + category |
| Anything else | extension fallback | Category by extension, never silently dropped |

Every analyzed file also gets a `nomic-embed-text` embedding for semantic search. Files are then matched against user-defined smart folders and (optionally) auto-moved when the match confidence is high enough.

Nothing leaves the machine.

## Setup

```bash
# 1. Install Ollama: https://ollama.com
# 2. Pull the three models Stratosort uses
ollama pull llama3.2:3b
ollama pull llava:7b
ollama pull nomic-embed-text
ollama serve   # if not already running

# 3. Clone + build
git clone https://github.com/iLevyTate/StratoSortRust.git
cd StratoSortRust/src-tauri
cargo build
```

Then either run the Tauri app (`npm run tauri dev` from the repo root once a frontend is wired up) or invoke the backend commands directly — they're all callable via `tauri::invoke` from any client.

### System requirements

- Rust 1.75+
- Ubuntu 22.04 / 24.04, macOS 11+, or Windows 10+
- 4 GB RAM minimum (8 GB recommended when llava is loaded)
- ~5 GB disk for the three Ollama models combined

### Linux build dependencies

The Tauri 2 crate links against webkit2gtk/javascriptcoregtk:

```bash
sudo apt-get install -y \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libjavascriptcoregtk-4.1-dev \
  libsoup-3.0-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

## How the AI dispatcher works

The core abstraction is `AiService::analyze_path_with_ai(path) -> FileAnalysis`. It looks at the extension and decides which Ollama model to call:

```
                 ┌─ jpg|jpeg|png|gif|bmp|webp ──► analyze_image  (llava)
                 │
analyze_path ────┼─ pdf|docx|xlsx|csv|md     ──► document_processor
                 │                                └─► analyze_file (llama)
                 │
                 ├─ plain text / source       ──► analyze_file   (llama)
                 │
                 └─ anything else             ──► extension fallback
```

Every successful analysis also generates and stores an embedding via `nomic-embed-text` for semantic search.

If Ollama is unreachable, the service falls back to extension-based classification so the file is still recorded — never silently dropped.

## Key Tauri commands

| Command | Purpose |
|---|---|
| `batch_analyze_files(paths)` | Run the dispatcher over a list of files |
| `reanalyze_files(paths)` | Clear cache + re-run analysis |
| `clear_stale_analyses()` | Purge fallback-stub rows from old runs |
| `semantic_search(query, limit)` | Hybrid: embeddings + tags + category + filename |
| `enable_watch_mode(directories)` | Auto-process new files dropped into folders |
| `disable_watch_mode()` | Stop auto-processing |
| `check_ollama_status()` | Connection + available models |
| `reconnect_ollama(host)` | Switch Ollama host at runtime |

## Watch mode

When enabled, Stratosort:

1. Walks every configured directory once and enqueues existing files (capped at 5000 files / depth 8).
2. Registers a `notify` watcher for future create events.
3. Runs the dispatcher on each file after a configurable debounce.
4. Moves matches to the highest-confidence smart folder if confidence ≥ threshold (default 0.7).

State persists across restarts — `Config.watch_folders` and `Config.watch_paths` are bridged into the runtime watcher at boot. The `auto_analyze_on_add` toggle lets users opt out of automatic AI processing while still tracking files.

## Project layout

```
src-tauri/
├── src/
│   ├── ai/              # AiService, OllamaClient, embeddings, dispatcher
│   ├── commands/        # Tauri command handlers (callable from frontend)
│   ├── core/            # Document/image/media processors
│   ├── services/        # FileWatcher, MonitoringService
│   ├── storage/         # SQLite + sqlite-vec extension wrappers
│   ├── state.rs         # AppState (Arc-everywhere, RwLock'd)
│   └── lib.rs           # Tauri setup + invoke handler registration
└── tests/               # Integration tests against tempfile-backed SQLite
```

## Development

```bash
cargo build                    # debug build
cargo test --lib               # 99 unit tests
cargo test --tests             # 127 integration tests (3 ignored — need Ollama)
cargo clippy --no-deps         # lints
cargo fmt                      # format
RUST_LOG=stratosort=debug cargo run   # verbose logs
```

## Privacy & safety

- All AI inference runs locally via Ollama — no API keys, no network egress for analysis.
- SQLite database lives under the platform's app-data dir (`%APPDATA%`, `~/Library/Application Support`, `~/.local/share`).
- File operations have undo/redo with full history.
- Prompt sanitization filters common injection patterns before content reaches the LLM.
- Path validation guards against traversal in every command that touches user paths.

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgements

- [Ollama](https://ollama.com/) — local model runtime
- [Tauri](https://tauri.app/) — desktop shell
- [sqlite-vec](https://github.com/asg017/sqlite-vec) — vector search inside SQLite
- [nomic-embed-text](https://ollama.com/library/nomic-embed-text), [llava](https://llava-vl.github.io/), [Llama 3.2](https://ai.meta.com/llama/) — the models that do the work
