# Whatszara — Desktop Assistant via WhatsApp + LLM

[![GitHub](https://img.shields.io/github/license/Preet3627/whatszara?style=flat-square&color=25D366)](LICENSE)

Control your desktop from anywhere using WhatsApp messages. Talk to an LLM through WhatsApp, and it executes your commands — shell, apps, media, file access — with a secure permission system and risk-based approval.

Built on top of **[whatsapp-mcp](https://github.com/lharries/whatsapp-mcp)** by Luke Harries, scaled from a simple MCP server into a full desktop automation platform.

**No Python required.** Everything is compiled into a single Tauri desktop app (Rust) + Go bridge for WhatsApp. No interpreters, no virtualenvs, no pip.


## Demo

[![Whatszara Demo]()](https://youtu.be/q5R8a360aGs)



## How It Works

```
WhatsApp Message ──▶ Orchestrator ──▶ LLM (Ollama/Claude/Groq/etc.)
                        │                        │
                        ▼                        ▼
                 Policy/Risk Check          Decides Action + Params
                        │                        │
                        └────────┬───────────────┘
                                 ▼
                    ┌─────────────────────┐
                    │  Risk Assessment     │
                    │  Low → Auto-execute  │
                    │  Med → User Approve  │
                    │  High → User Confirm │
                    └──────────┬──────────┘
                               ▼
                        System Action (shell, apps, media...)
                               │
                               ▼
                        Result sent back via WhatsApp
```

## Features

### ✅ Completed
- [x] Python eliminated — everything in Rust + Go (zero Python dependency)
- [x] Multi-LLM provider abstraction (Ollama, Claude, Groq, Grok, Gemini) in Rust
- [x] Live model list fetching for all 5 providers via their REST APIs
- [x] Tauri desktop app with system tray and 6-tab dashboard
- [x] Policy engine with 3 risk profiles (High/Medium/Low)
- [x] Per-tool permissions (independently toggle shell, file, media, apps, WhatsApp)
- [x] Structured action types with propose → evaluate → execute flow
- [x] WhatsApp account allowlist + per-contact mode (Assistant/Chat/Summarize/Blocked)
- [x] GUI contacts table with search, allowlist toggle, mode dropdown
- [x] Built-in chat view with message history and live 3-second auto-polling
- [x] AI reply capability from chat view with Enter-to-send
- [x] Risk/approval system: AI-triggered tool calls with approve/reject in chat UI
- [x] Pending actions panel with Approve/Reject buttons and badge counter
- [x] Shell command executor with blocklist (disabled by default)
- [x] App launcher, volume control, media playback (macOS)
- [x] Desktop image scanner
- [x] Reversible undo journal for all actions
- [x] Permanent WhatsApp auth via platform-native credential store (auto-save + restore)
- [x] Persistent policy config in credential store (allowlist, modes, permissions)
- [x] Configurable Ollama endpoint from GUI
- [x] API key management for cloud providers from GUI
- [x] API_KEY env var auth on Go bridge endpoints
- [x] Logout button to clear auth and keychain entries
- [x] WhatsApp MCP tools in Rust (SQLite reads + HTTP to Go bridge)
- [x] Setup.sh one-click bootstrap
- [x] Multi-platform CI + Release workflows (GitHub Actions)
- [x] MIT License with whatsapp-mcp attribution

### 📋 Planned
- [ ] reCAPTCHA + image-to-text verification integration
- [ ] Scheduled/automated actions
- [ ] Multiple WhatsApp number support
- [ ] Voice message transcription
- [ ] Plugin system for custom tools

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Whatszara                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────────────┐      ┌─────────────────────────────────┐    │
│  │   WhatsApp Layer        │      │    Tauri Desktop App (Rust)      │    │
│  │   (Go Bridge)           │─────▶│                                  │    │
│  │   - whatsmeow client    │      │  ┌──────────────────────────┐   │    │
│  │   - SQLite msg store    │      │  │  LLM Providers           │   │    │
│  │   - REST API :8080      │      │  │  - Ollama (live fetch)   │   │    │
│  │   - API_KEY auth        │      │  │  - Claude/Groq/Grok/Gem  │   │    │
│  └──────────┬──────────────┘      │  └──────────────────────────┘   │    │
│             │                     │                                  │    │
│             │                     │  ┌──────────────────────────┐   │    │
│   ┌─────────▼──────────┐         │  │  Policy Engine           │   │    │
│   │  SQLite (messages)  │◀────────│  │  - 3 risk profiles      │   │    │
│   │  + contacts table   │         │  │  - Per-tool permissions │   │    │
│   └─────────────────────┘         │  │  - Allowlist + modes    │   │    │
│                                   │  └──────────────────────────┘   │    │
│                                   │                                  │    │
│  ┌────────────────────────┐      │  ┌──────────────────────────┐   │    │
 │  │  Credential Store       │      │  │  Action Engine          │   │    │
│  │  - WA session (auto)   │      │  │  - Shell (disabled)     │   │    │
│  │  - Config (allowlist   │      │  │  - macOS: osascript     │   │    │
│  │    modes, perms)       │      │  │  - Volume / Media       │   │    │
│  └────────────────────────┘      │  │  - File scanner         │   │    │
│                                   │  │  - Undo journal        │   │    │
│                                   │  └──────────────────────────┘   │    │
│                                   │                                  │    │
│                                   │  ┌──────────────────────────┐   │    │
│                                   │  │  Risk/Approval System    │   │    │
│                                   │  │  - Tool call parsing     │   │    │
│                                   │  │  - Pending actions queue │   │    │
│                                   │  │  - Approve/Reject UI     │   │    │
│                                   │  └──────────────────────────┘   │    │
│                                   │                                  │    │
│                                   │  ┌──────────────────────────┐   │    │
│                                   │  │  Frontend (HTML/JS)      │   │    │
│                                   │  │  - Dashboard + Wizard   │   │    │
│                                   │  │  - Chat view + polling  │   │    │
│                                   │  │  - Permissions table    │   │    │
│                                   │  │  - Provider config      │   │    │
│                                   │  │  - Settings + Keychain  │   │    │
│                                   │  └──────────────────────────┘   │    │
│                                   └─────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Requirements

- **Go** for the WhatsApp bridge
- **Node.js 20+** and **Rust** for the Tauri desktop app
- **Ollama** or an API key for Claude, Groq, Grok, or Gemini
- **FFmpeg** optional, only needed for audio-message workflows

**Python is NOT required.**

### Install

```bash
chmod +x setup.sh && ./setup.sh
```

Or run the project setup target directly:

```bash
make setup
```

### Launch

```bash
make desktop
```

The desktop app opens with a modern setup wizard, live bridge status, light/dark/vibrant themes, keyboard shortcuts, and a built-in Guide tab for help.

## First-Run Setup

### 1. Connect WhatsApp

The app starts the bridge automatically from `whatsapp-bridge/`. When the QR code appears, open WhatsApp on your phone:

```text
Linked Devices -> Link a Device -> Scan QR
```

The dashboard shows bridge states in real time:

| Status | Meaning |
|--------|---------|
| `stopped` | The bridge is not running |
| `starting...` | The bridge process is booting |
| `scan QR` | WhatsApp needs device linking |
| `connected` | The bridge API is reachable and authenticated |
| `error` | The bridge failed; open the dashboard error detail |

### 2. Choose an LLM Provider

Open **Providers**, choose the active provider, and refresh model lists. Ollama works locally; cloud providers require API keys.

```bash
export OLLAMA_ENDPOINT=http://localhost:11434
export ANTHROPIC_API_KEY=sk-ant-...
export GROQ_API_KEY=gsk-...
export XAI_API_KEY=xai-...
export GEMINI_API_KEY=AIza...
```

You can also paste provider keys into **Settings** and save local settings from the app.

### 3. Allowlist Contacts

Open **Permissions**, review contacts, and allowlist only trusted WhatsApp JIDs. Contact modes control behavior:

| Mode | Behavior |
|------|----------|
| **Assistant** | Can request desktop actions through the LLM |
| **Chat** | Text-only responses, no action execution |
| **Summarize** | Produces short summaries |
| **Blocked** | Rejected by policy |

### 4. Send a Message

Send a WhatsApp message to the connected account. Whatszara reads the message, asks the active model what to do, checks policy, and logs the result.

## Desktop UI Guide

### Themes

Use the sidebar theme switcher:

| Theme | Best for |
|-------|----------|
| **Dark** | Default focused workspace |
| **Light** | Bright rooms and screenshots |
| **Vibrant** | High-contrast colorful dashboard |

Theme choice is saved in local storage and restored on launch.

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + 1` | Dashboard |
| `Cmd/Ctrl + 2` | Chat |
| `Cmd/Ctrl + 3` | Providers |
| `Cmd/Ctrl + 4` | Permissions |
| `Cmd/Ctrl + 5` | Action Log |
| `Cmd/Ctrl + 6` | Settings |
| `Cmd/Ctrl + 7` | Guide |
| `Cmd/Ctrl + K` | Focus search or reply |
| `Cmd/Ctrl + J` | Open Chat |
| `Cmd/Ctrl + G` or `?` | Open Guide |
| `Esc` | Clear and blur the focused input |

## Troubleshooting

| Problem | Fix |
|---------|-----|
| Bridge shows `stopped` | Confirm Go is installed with `go version`, then restart the app |
| Bridge shows `error` | Run `cd whatsapp-bridge && go run main.go` to see raw bridge logs |
| `go: not found` | Install Go from [go.dev](https://go.dev/dl/) and confirm it is in `PATH` |
| QR code does not appear | Remove `whatsapp-bridge/store/`, restart, and link again |
| QR scan fails repeatedly | Confirm your phone has internet and WhatsApp Linked Devices is available |
| Port `8080` is busy | Stop the other process or change the bridge port in `whatsapp-bridge/main.go` |
| Ollama models are missing | Run `ollama serve`, then `ollama pull llama3.1`, then refresh Providers |
| Cloud provider fails | Check the matching API key in Settings or shell environment |
| Messages do not execute | Check allowlist, contact mode, active provider, and tool permission toggles |
| Actions stay pending | Open Chat, select the contact, then approve or reject pending actions |

### Manual Bridge Mode

Use this when debugging bridge logs separately:

```bash
# Terminal 1
make bridge

# Terminal 2
make desktop
```

## Policy & Permission System

Whatszara uses a **propose → evaluate → execute** flow with risk-based approval.

### Risk Profiles

| Risk Level | Examples | Approval Required |
|-----------|----------|------------------|
| **Low** | Read volume, list files | None (auto-execute) |
| **Medium** | Open apps, play music, set volume | User approve in chat UI |
| **High** | Shell commands, delete, install | User approve in chat UI |

### Per-Tool Permissions

| Category | Default | Actions |
|----------|---------|---------|
| Shell | **Disabled** | `execute_shell`, `run_command` |
| File Access | Enabled | `list_files`, `list_images`, `get_desktop_paths` |
| Media Control | Enabled | `get_volume`, `set_volume`, `play`, `pause` |
| App Launching | Enabled | `open_app` |
| WhatsApp | Enabled | `send_message`, `search_contacts` |

### Contact Modes

Every allowed contact has a mode:
- **Assistant** — Full AI control (tool calls + approve/reject)
- **Chat** — Text only, no desktop actions
- **Summarize** — 2-3 sentence summary (default)
- **Blocked** — Ignored at policy level

## Persistent Storage

| What | Where | How |
|------|-------|-----|
| WhatsApp session | Credential Store (`whatszara-wa-session`) | Auto-saved on first connect, auto-restored on launch |
| Policy config | Credential Store (`whatszara-config`) | Auto-saved on every change, manual load from Settings |
| App settings | Browser localStorage | Endpoint URLs, API keys |

## Chat View & AI Replies

The built-in chat view features:
- **Left panel**: Searchable contact list sorted by allowlisted status (allowlisted contacts first)
- **Right panel**: Message history with timestamps, auto-scroll to newest
- **3-second auto-polling** for live updates
- **Reply area**: Type a message, AI processes and sends response via WhatsApp. Only visible for allowlisted contacts
- **Pending actions panel**: Shows AI-triggered tool calls with Approve/Reject buttons. High-risk actions require approval before execution

## Credential Storage

Whatszara uses the **[keyring](https://github.com/hwchen/keyring-rs)** crate for cross-platform credential storage — no platform-specific code needed.

| Platform | Backend |
|----------|---------|
| macOS | iCloud Keychain (via Security framework) |
| Windows | Credential Manager (via wincred) |
| Linux | Secret Service / keyutils |

Two entries are stored with service name and username `whatszara`:

- **`whatszara-wa-session`** — WhatsApp session DB (base64-encoded). Auto-saved on first connect, restored on startup — no QR re-scan needed
- **`whatszara-config`** — Policy config (allowlist, contact modes, tool permissions). Auto-saved on every change, auto-restored on startup

**Logout**: Kills the bridge, deletes both credential entries, removes session file. Click "Logout & Disconnect" on the Dashboard.

## License

MIT Licensed. © 2026 Preet3627 (Latestinssan). The WhatsApp bridge incorporates code from [whatsapp-mcp](https://github.com/lharries/whatsapp-mcp) by Luke Harries.

## Documentation

Full docs site: **[github.com/Preet3627/whatszara-docs](https://whatszara.vercel.app/)**
