<p align="center">
  <img src="docs/icon.png" width="128" alt="Magisterium icon" />
</p>

<h1 align="center">Magisterium</h1>

<p align="center">
  <strong>Convene a council of AIs.</strong><br />
  Claude, Gemini, your local models and any API answer the same question,<br />
  critique each other, and hand you one verdict.
</p>

<p align="center">
  <img alt="macOS" src="https://img.shields.io/badge/macOS-Apple%20Silicon-111?logo=apple&logoColor=white" />
  <img alt="Linux and Windows" src="https://img.shields.io/badge/Linux%20·%20Windows-experimental-555" />
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" />
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?logo=rust&logoColor=white" />
  <img alt="React" src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" />
  <img alt="Languages" src="https://img.shields.io/badge/UI-EN%20·%20FR%20·%20ES%20·%20AR-a48bff" />
  <a href="LICENSE"><img alt="License: AGPL-3.0" src="https://img.shields.io/badge/license-AGPL--3.0-blue" /></a>
</p>

<p align="center">
  <img src="docs/screenshots/council.png" alt="The council's verdict on top, then each AI's answer one tab at a time" />
</p>

---

## Why a council?

One model gives you one answer, blind spots included. Magisterium runs **several models in parallel**, lets them **read and critique each other's answers**, then has one of them **write the final synthesis**. It keeps what everyone got right, fixes what someone got wrong, and settles the disagreements.

In practice it catches the stuff a single model gets confidently wrong: an outdated fact that another model corrects, or a missing edge case that a reviewer points out.

|                       | One AI         | The council                               |
| --------------------- | -------------- | ----------------------------------------- |
| Answers               | 1              | 2 to 5 in parallel, plus a synthesis      |
| Errors                | Go unnoticed   | Flagged by the other members              |
| Disagreements         | Invisible      | Surfaced and settled explicitly           |
| Your subscriptions    | One at a time  | Claude Code, Antigravity, APIs and local models side by side |

## Features

- 🧙 **Council modes.** *Compare* sends the same prompt to every member at once. *Debate* adds 2 to 5 rounds of cross-review, where each AI reads the others, critiques them and revises its own answer. An optional **final synthesis** can be written by any model you choose.
- 💬 **Real conversations.** Follow-up messages carry the whole thread (your prompts and the council's syntheses), so the council builds on its previous conclusions. Every discussion is saved locally and can be reopened.
- 🔌 **Bring your own AIs.**
  - **CLIs**: **Claude Code CLI** and **Antigravity CLI** are detected automatically and use your existing subscriptions.
  - **Local servers**: LM Studio, Ollama, or any OpenAI-compatible URL, including another machine on your network.
  - **Cloud APIs**: Anthropic, OpenAI, Gemini, DeepSeek, OpenRouter, Mistral, Groq, xAI, Together, Ollama Cloud.
- 🧠 **Per-model control.** Pick the model and the **thinking effort** (low → max for Claude) for each member. Add the same provider twice with different models if you like.
- 🌐 **Web access for every model.** API and local models get `web_search` and `fetch_url` tools, executed by the app. Search runs through a **built-in SearXNG**, Tavily, or your own SearXNG instance.
- 📊 **Usage limits at a glance.** Claude Code and Antigravity 5-hour and weekly windows, OpenRouter and DeepSeek balances, Tavily credits, each with a reset countdown. A mini gauge on every council member warns you before you hit a wall. Checking is free: no tokens spent.
- 👀 **See them think.** Answers stream live. Reasoning (when the provider exposes it), searches, pages and files read appear with each answer.
- 🏛️ **Verdict first.** Each turn opens on the council's verdict. The debate sits right below: one tab per AI with its full-width answer, or a side-by-side view to compare.
- 📁 **Your project, for every model.** Pick a workspace folder: CLI agents work in it with their own tools, and API or local models get read-only `list_dir`, `read_file` and `search_files` tools, confined to that folder.
- 🛡️ **Execution modes for agents**: *Plan* (read-only), *Edit*, *Auto*, *Full access*, applied to the CLI agents working in your project folder.
- 🍏 **Native feel.** Liquid-glass UI on real macOS vibrancy, collapsible side panels (`⌘B` / `⌘J`), and animations tuned with Emil Kowalski's and Apple's motion principles. Reduced-motion and reduced-transparency are respected.
- 🌍 **Four languages**: English, Français, Español, العربية (right-to-left). The instructions sent to the models follow the UI language too.

## How a session works

```mermaid
flowchart LR
    Q([Your question<br/>+ discussion history]) --> A1[Claude Code CLI]
    Q --> A2[Antigravity CLI]
    Q --> A3[Local / API model]
    A1 & A2 & A3 --> R{{Cross-review<br/>each AI reads the others,<br/>critiques and revises}}
    R -->|repeat 1–4×| R
    R --> S[[Synthesis<br/>by the model you pick]]
    S --> V([One answer])
    A3 -. web_search / fetch_url .-> W[(SearXNG · Tavily)]
```

## Screenshots

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/live.png" alt="A turn in progress" /><br /><sub><b>Live deliberation:</b> answers stream in while the verdict waits for the debate to finish.</sub></td>
    <td width="50%"><img src="docs/screenshots/side-by-side.png" alt="Answers side by side" /><br /><sub><b>Side by side:</b> equal-size cards that scroll horizontally, to compare every answer at a glance.</sub></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/screenshots/web-tools.png" alt="A model searching the web and showing its reasoning" /><br /><sub><b>Web tools & reasoning:</b> an API model searches, reads a page, and shows its chain of thought.</sub></td>
    <td width="50%"><img src="docs/screenshots/thread.png" alt="Discussion with debates collapsed" /><br /><sub><b>Threaded discussions:</b> collapse the debates and read it like a chat, one verdict per turn.</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/settings.png" alt="Settings" /><br /><sub><b>Settings:</b> language, detected CLIs, configured providers and web search engine.</sub></td>
    <td><img src="docs/screenshots/providers.png" alt="Provider presets" /><br /><sub><b>Add a provider:</b> local servers and cloud presets, with a connection test before saving.</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/usage.png" alt="Usage limits" /><br /><sub><b>Usage limits:</b> subscription windows, API balances and reset times, read at no cost.</sub></td>
    <td><img src="docs/screenshots/empty.png" alt="Empty state" /><br /><sub><b>Ready to convene:</b> your council, its deliberation mode and workspace on the right.</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/searxng.png" alt="Built-in SearXNG installation" /><br /><sub><b>Built-in SearXNG:</b> installed on demand, with every step and live download progress.</sub></td>
    <td><img src="docs/screenshots/focus.png" alt="Focus mode" /><br /><sub><b>Focus mode:</b> both panels collapsed with <code>⌘B</code> / <code>⌘J</code>.</sub></td>
  </tr>
</table>

## Providers

| Type | Providers | Notes |
| --- | --- | --- |
| **CLI agents** | Claude Code CLI, Antigravity CLI | Auto-detected, no API key: they use your subscriptions. They have their own tools (files, shell, web). |
| **Local servers** | LM Studio, Ollama, custom URL | OpenAI-compatible, optional API key. Works across your LAN. LM Studio models that fail to *just-in-time* load are loaded explicitly, and already-loaded models are marked `●`. |
| **Cloud APIs** | Anthropic, OpenAI, Google Gemini, DeepSeek, OpenRouter, Mistral, Groq, xAI, Together AI, Ollama Cloud | One key per provider, stored in the **macOS Keychain**. |

## Web search

Models reached through an API don't browse by themselves: the app gives them two tools and runs them on their behalf.

| Tool | What it does | Cost |
| --- | --- | --- |
| `fetch_url` | Downloads a page and keeps only its readable text | Free |
| `web_search` | Queries the selected search engine | Depends on the engine, see below |

| Engine | Setup | Cost |
| --- | --- | --- |
| **Built-in SearXNG** *(recommended)* | One click in Settings. The app downloads `uv`, a standalone Python and SearXNG into its own folder (~220 MB) and runs it on `127.0.0.1` only. **No Python or other software required.** | Free, no quota |
| **Tavily** | Paste an API key | 1,000 free searches / month |
| **Your SearXNG** | Paste its URL (JSON format enabled) | Free |

> Nothing is downloaded until you click **Install**, and the built-in engine can be disabled or uninstalled at any time.

## Execution modes

These apply to the CLI agents, which can read and edit files in the workspace folder you choose. API and local models can only **read** that folder.

| Mode | Claude Code CLI | Antigravity CLI |
| --- | --- | --- |
| **Plan** | `--permission-mode plan` (read-only) | `--mode plan` |
| **Edit** | `acceptEdits` | `accept-edits` |
| **Auto** | `auto`: Claude decides what is safe | falls back to `accept-edits` |
| **Full** | `--dangerously-skip-permissions` | `--dangerously-skip-permissions` |

## Privacy

- **Stays on your Mac**: discussions, provider settings and the council setup (`~/Library/Application Support/dev.magisterium.app`).
- **API keys** live in a single macOS Keychain item, never in a file.
- **Built-in SearXNG** listens on `127.0.0.1` only.
- **What goes out**: your prompts are sent only to the providers you add to the council.

## Install

1. Download the latest `Magisterium_x.y.z_aarch64.dmg` and drag **Magisterium** into Applications.
2. The app is not notarized yet: on first launch, **right-click → Open**.
3. To reach a server on your local network, allow Magisterium in **System Settings › Privacy & Security › Local Network**.

The CLI members need their own tools installed and logged in: [`claude`](https://docs.anthropic.com/en/docs/claude-code) and/or `agy` (Antigravity).

## Build from source

Requirements: [Rust](https://rustup.rs) and Node.js 20+.

```bash
npm install
npm run tauri dev        # development
```

Release builds run on the target system (Tauri doesn't cross-compile desktop apps):

| System | Command | Output |
| --- | --- | --- |
| macOS | `scripts/build-macos.sh` (`--universal` for Apple Silicon + Intel) | `.app`, `.dmg` |
| Linux | `scripts/build-linux.sh` (`--bundles deb` to pick formats) | `.deb`, `.rpm`, `.AppImage` |
| Windows | `.\scripts\build-windows.ps1` (`-Bundles nsis` to pick) | `.msi`, `-setup.exe` |

Each script checks its prerequisites first (Xcode tools, WebKitGTK and D-Bus headers, MSVC Build Tools) and prints the install command if something is missing. Shortcuts: `npm run build:mac`, `build:linux`, `build:win`.

Clean up build artifacts (≈ 10 GB after a few builds). App data (discussions, providers, keys) is never touched:

```bash
scripts/clean.sh             # dist/, src-tauri/target/, src-tauri/gen/
scripts/clean.sh --bundles   # only the produced packages
scripts/clean.sh --all       # also node_modules/
scripts/clean.sh --dry-run   # show what would be removed
.\scripts\clean.ps1 -DryRun  # Windows equivalent (-Bundles, -All)
```

Tests:

```bash
cd src-tauri
cargo test                                            # unit tests
cargo test -- --ignored live_debate --nocapture       # real debate: Claude CLI + Antigravity CLI
cargo test -- --ignored live_builtin_searxng --nocapture   # full SearXNG install in a temp folder
```

Regenerate the README screenshots (headless Chromium, mocked backend, no real API calls):

```bash
npm run screenshots
```

## Project structure

```
src/                      React front-end
  App.tsx                 layout, council panel, streaming, persistence
  TurnView.tsx            a turn: debate rounds, tool uses, reasoning, synthesis
  Settings.tsx            languages, providers, web search, SearXNG installer
  i18n.tsx                EN · FR · ES · AR
src-tauri/src/            Rust back-end
  orchestrator.rs         compare → cross-review rounds → synthesis, retries
  prompts.rs              debate / synthesis prompts in the four languages
  providers/              Claude CLI, Antigravity CLI, OpenAI-compatible, Anthropic
  usage.rs                usage limits: CLI /usage windows, API balances
  tools.rs                web_search & fetch_url
  searxng.rs              on-demand SearXNG install and lifecycle
  config.rs · history.rs  providers, Keychain vault, saved discussions
scripts/screenshots/      mocked backend + capture script for this README
```

## Platforms

**macOS** is where Magisterium is built and tested every day. **Linux and Windows** builds are wired up (native keychains, Mica on Windows 11, SearXNG binaries per platform, `.cmd` CLI shims), but they haven't been run on real machines yet. Reports welcome.

## Roadmap

- Web tools for Anthropic-protocol providers
- Signed and notarized builds (Touch ID for the Keychain, no Gatekeeper prompt)
- OAuth providers (GitHub Copilot, ChatGPT subscription)
- Export a discussion to Markdown
- Tested Linux and Windows releases

## License

Magisterium is free software, released under the [GNU Affero General Public License v3.0 or later](LICENSE).
You can use, study, modify and share it. If you distribute a modified version, or run one as a network service, you must publish its source code under the same license.

Copyright © 2026 Okil Lakhdari.

## Credits

Built with [Tauri](https://tauri.app), [React](https://react.dev) and [Rust](https://www.rust-lang.org). Web search powered by [SearXNG](https://github.com/searxng/searxng), installed with [uv](https://github.com/astral-sh/uv) and [python-build-standalone](https://github.com/astral-sh/python-build-standalone). The idea of letting several AIs deliberate together was sparked by PewDiePie's *Odysseus*.
