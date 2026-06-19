# Whatszara — Desktop Assistant via WhatsApp + LLM

Control your desktop from anywhere using WhatsApp messages. Talk to an LLM through WhatsApp, and it executes your commands — shell, apps, media, file access — with a secure permission system.

Built on top of **[whatsapp-mcp](https://github.com/lharries/whatsapp-mcp)** by Luke Harries, scaled from a simple MCP server into a full desktop automation platform.

**No Python required.** Everything is compiled into a single Tauri desktop app (Rust) + Go bridge for WhatsApp. No interpreters, no virtualenvs, no pip.

## How It Works

```
WhatsApp Message ──▶ Orchestrator ──▶ LLM (Ollama/Claude/Groq/etc.)
                        │                        │
                        ▼                        ▼
                 Permission Check          Decides Action
                        │                        │
                        └────────┬───────────────┘
                                 ▼
                        System Action (shell, apps, media...)
                                 │
                                 ▼
                        Result sent back via WhatsApp
```

## The Scaling Story: from whatsapp-mcp to Whatszara

[whatsapp-mcp](https://github.com/lharries/whatsapp-mcp) is a focused MCP server with **12 tools** that lets Claude read and send WhatsApp messages. It's:
- **2 components**: Go bridge + Python MCP server
- **3 source files**: `main.go`, `main.py`, `whatsapp.py`
- **Single-direction**: LLM talks TO WhatsApp

**Whatszara inverts this.** WhatsApp messages trigger the LLM to control the desktop. Here's what we added:

| Dimension | whatsapp-mcp | Whatszara |
|-----------|-------------|-----------|
| **Message flow** | LLM → WhatsApp | WhatsApp → LLM → Desktop |
| **LLM support** | Claude only | Ollama, Claude, Groq, Grok, Gemini, Vercel AI SDK |
| **Actions** | None (read/send only) | Shell, open apps, volume, media, file scan, send images |
| **Permissions** | None | reCAPTCHA + image-to-text, 3 risk tiers, undo system |
| **Interface** | CLI config only | Tauri desktop app with GUI |
| **Scope** | WhatsApp tool | Full desktop assistant |

## Features

### ✅ Completed
- [x] Python eliminated — everything in Rust + Go (zero Python dependency)
- [x] Multi-LLM provider abstraction (Ollama, Claude, Groq, Grok, Gemini, Vercel AI SDK) in Rust
- [x] Live model list fetching for Ollama
- [x] Tauri desktop app with system tray
- [x] Permission engine with 3 risk profiles (High/Medium/Low)
- [x] Shell command executor with blocklist
- [x] App launcher with aliases
- [x] Volume control + media playback (macOS)
- [x] Desktop image scanner
- [x] Reversible undo journal for all actions
- [x] WhatsApp MCP tools in Rust (SQLite reads + HTTP to Go bridge)
- [x] Setup.sh one-click bootstrap
- [x] Multi-platform CI + Release workflows (GitHub Actions)
- [x] MIT License with whatsapp-mcp attribution

### 🔄 In Progress
- [ ] Image-to-text + reCAPTCHA verification integration
- [ ] WhatsApp incoming message webhook
- [ ] Permission configuration GUI
- [ ] Action history viewer

### 📋 Planned
- [ ] Scheduled/automated actions
- [ ] Multiple WhatsApp number support
- [ ] Voice message transcription
- [ ] Plugin system for custom tools

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        Whatszara                                 │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌─────────────────────┐      ┌──────────────────────────────┐   │
│  │   WhatsApp Layer    │      │    Tauri Desktop App (Rust)   │   │
│  │   (Go Bridge)       │─────▶│                               │   │
│  │   - whatsmeow       │      │  ┌─────────────────────────┐  │   │
│  │   - SQLite store    │      │  │  LLM Providers (Rust)   │  │   │
│  │   - REST API :8080  │      │  │  - Ollama (live list)   │  │   │
│  └──────────┬──────────┘      │  │  - Claude               │  │   │
│             │                 │  │  - Groq / Grok / Gemini │  │   │
│             │                 │  └─────────────────────────┘  │   │
│             │                 │                               │   │
│             │                 │  ┌─────────────────────────┐  │   │
│   ┌─────────▼──────────┐     │  │  Permission Engine       │  │   │
│   │  SQLite (Messages)  │◀────│  │  - 3 risk profiles      │  │   │
│   │  (Rust reads dir.)  │     │  │  - reCAPTCHA + Image-txt│  │   │
│   └─────────────────────┘     │  └─────────────────────────┘  │   │
│                               │                               │   │
│                               │  ┌─────────────────────────┐  │   │
│                               │  │  Action Engine (Rust)    │  │   │
│                               │  │  - Shell via Command     │  │   │
│                               │  │  - macOS: osascript      │  │   │
│                               │  │  - Volume / Media        │  │   │
│                               │  │  - File scanner          │  │   │
│                               │  │  - Undo journal          │  │   │
│                               │  └─────────────────────────┘  │   │
│                               │                               │   │
│                               │  ┌─────────────────────────┐  │   │
│                               │  │  Frontend (Svelte/HTML)  │  │   │
│                               │  │  - Dashboard            │  │   │
│                               │  │  - Provider config      │  │   │
│                               │  │  - Permissions editor   │  │   │
│                               │  │  - Action log           │  │   │
│                               │  └─────────────────────────┘  │   │
│                               └──────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites
- **Go** (for WhatsApp bridge)
- **Node.js 20+** + **Rust** (for Tauri desktop app)
- **FFmpeg** (optional — for audio messages)

**Python is NOT required.**

### Setup (30 seconds)

```bash
# One-command setup — installs everything
chmod +x setup.sh && ./setup.sh

# Or manually:
make setup
```

### Run

```bash
# Terminal 1: Start WhatsApp bridge
make bridge
# Scan QR code with WhatsApp mobile app

# Terminal 2: Start desktop app
make desktop
```

### Configuring LLM Providers

Set environment variables OR configure in the desktop app GUI:

```bash
# Ollama (default — works out of the box)
export OLLAMA_ENDPOINT=http://localhost:11434

# Claude
export ANTHROPIC_API_KEY=sk-ant-...

# Groq
export GROQ_API_KEY=gsk-...

# Grok (xAI)
export XAI_API_KEY=...

# Gemini
export GEMINI_API_KEY=...
```

## Permission System

| Risk Level | Example Actions | Verification Required |
|-----------|----------------|---------------------|
| **Low** | Read volume, list files, get time | None (logged only) |
| **Medium** | Open apps, play music, send files | Image-to-text CAPTCHA |
| **High** | Shell commands, delete, install software | reCAPTCHA + image-to-text + confirm |

## License

This project is **MIT Licensed** (see [LICENSE](LICENSE)).

The WhatsApp bridge and MCP server components incorporate code from
[whatsapp-mcp](https://github.com/lharries/whatsapp-mcp) by Luke Harries,
also MIT licensed. See [LICENSE-THIRD-PARTY](LICENSE-THIRD-PARTY) for attribution.
