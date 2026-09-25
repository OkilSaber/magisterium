// Données de démonstration pour les captures d'écran du README.

const now = Date.UTC(2026, 8, 25, 16, 20);

export const providers = [
  {
    id: "claude-cli",
    name: "Claude Code CLI",
    preset: "cli",
    available: true,
    status: null,
    uses_tools: true,
    efforts: ["low", "medium", "high", "xhigh", "max"],
    models: [
      { id: "", label: "Default" },
      { id: "opus", label: "Opus" },
      { id: "sonnet", label: "Sonnet" },
      { id: "haiku", label: "Haiku" },
    ],
  },
  {
    id: "antigravity-cli",
    name: "Antigravity CLI",
    preset: "cli",
    available: true,
    status: null,
    uses_tools: true,
    efforts: [],
    models: [
      { id: "", label: "Default" },
      { id: "gemini-3.1-pro-high", label: "Gemini 3.1 Pro (High)" },
      { id: "gemini-3.8-flash-high", label: "Gemini 3.8 Flash (High)" },
    ],
  },
  {
    id: "lan-lmstudio",
    name: "Desktop · LM Studio",
    preset: "lmstudio",
    available: true,
    status: null,
    uses_tools: false,
    efforts: [],
    models: [
      { id: "google/gemma-4-e2b", label: "Gemma 4 E2B", loaded: true },
      { id: "essentialai/rnj-1", label: "Rnj 1", loaded: false },
    ],
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    preset: "openrouter",
    available: true,
    status: null,
    uses_tools: false,
    efforts: [],
    models: [{ id: "deepseek/deepseek-v4.1-flash", label: "DeepSeek V4.1 Flash" }],
  },
];

export const providerConfigs = [
  { id: "lan-lmstudio", name: "Desktop · LM Studio", kind: "openai", preset: "lmstudio", base_url: "http://192.168.1.40:1234/v1", has_key: false },
  { id: "openrouter", name: "OpenRouter", kind: "openai", preset: "openrouter", base_url: "https://openrouter.ai/api/v1", has_key: true },
];

export const council = {
  selections: [
    { provider: "claude-cli", model: "opus", effort: "high" },
    { provider: "antigravity-cli", model: "gemini-3.1-pro-high", effort: "" },
    { provider: "openrouter", model: "deepseek/deepseek-v4.1-flash", effort: "" },
  ],
  mode: "debate",
  rounds: 2,
  synthOn: true,
  synth: { provider: "claude-cli", model: "opus" },
};

const agents = [
  { key: "a1", label: "Claude Code CLI · Opus", provider: "claude-cli", model: "opus", effort: "high" },
  { key: "a2", label: "Antigravity CLI · Gemini 3.1 Pro (High)", provider: "antigravity-cli", model: "gemini-3.1-pro-high", effort: "" },
  { key: "a3", label: "OpenRouter · DeepSeek V4.1 Flash", provider: "openrouter", model: "deepseek/deepseek-v4.1-flash", effort: "" },
];
const synthesizer = { key: "synthese", label: "Claude Code CLI · Opus", provider: "claude-cli", model: "opus", effort: "" };

const done = (text: string, extra: object = {}) => ({ text, status: "done", ...extra });

export const conversation = {
  id: "demo-rust-go",
  title: "Rust or Go for our new CLI tool?",
  created_at: now - 3_600_000,
  updated_at: now,
  turns: [
    {
      id: "t1",
      prompt: "We're starting a new internal CLI tool at work. Rust or Go?",
      created_at: now - 3_600_000,
      mode: "debate",
      agents,
      synthesizer,
      synth_round: 3,
      status: "done",
      rounds: [
        {
          round: 1,
          label: "",
          outputs: {
            a1: done("**Go**, for most teams. Fast to learn, fast to ship, great CLI libraries (`cobra`, `bubbletea`)."),
            a2: done("**Rust.** `clap` + `ratatui` give you a best-in-class CLI, and the compiler catches whole classes of bugs."),
            a3: done("It depends on the team's experience and on how performance-critical the tool is."),
          },
        },
        {
          round: 2,
          label: "",
          outputs: {
            a1: done("Gemini is right about safety, but for an internal tool, time-to-ship wins. **Go.**"),
            a2: done("Fair point on onboarding. If nobody knows Rust, **Go** is the pragmatic choice."),
            a3: done("Both agree on Go for velocity; Rust only if the tool sits on a hot path."),
          },
        },
        {
          round: 3,
          label: "",
          outputs: {
            synthese: done(
              "**Go.** The council converged: for an internal tool built by a mixed team, Go's gentle learning curve and fast iteration outweigh Rust's stronger guarantees. Revisit Rust if a component becomes performance-critical.",
            ),
          },
        },
      ],
    },
    {
      id: "t2",
      prompt: "OK, Go it is. How should we structure the project so it stays maintainable?",
      created_at: now - 600_000,
      mode: "debate",
      agents,
      synthesizer,
      synth_round: 3,
      status: "done",
      rounds: [
        {
          round: 1,
          label: "",
          outputs: {
            a1: done(
              "## Layout\n\n```\ncmd/tool/main.go\ninternal/\n  config/\n  commands/\n  api/\n```\n\n- **`cobra`** for commands, one file per command\n- Keep `main.go` tiny: parse flags, call `internal/commands`\n- Everything in `internal/` so nothing leaks as a public API",
            ),
            a2: done(
              "## Recommended structure\n\n| Folder | Purpose |\n|---|---|\n| `cmd/` | entry point |\n| `internal/cli` | commands & flags |\n| `internal/core` | business logic, no I/O |\n| `internal/adapters` | HTTP, files, env |\n\nKeep `core` pure so it's trivially testable.",
            ),
            a3: done(
              "I checked current community guidance:\n\n1. Follow the standard **`cmd/` + `internal/`** split.\n2. Use **`cobra`** + **`viper`** for commands and config.\n3. Ship with **GoReleaser** for cross-platform binaries.\n\nThe 2026 Go survey still lists cobra as the most used CLI framework.",
              {
                tools: [
                  { name: "web_search", detail: "go cli project structure best practices 2026" },
                  { name: "fetch_url", detail: "https://go.dev/doc/modules/layout" },
                ],
                thinking:
                  "The user wants maintainability, not just a folder list. Let me check the official Go module layout guidance before answering, then add release tooling since it's a CLI…",
              },
            ),
          },
        },
        {
          round: 2,
          label: "",
          outputs: {
            a1: done(
              "## Feedback\n\n- **Gemini**'s `core` / `adapters` split is a real improvement over my flat `internal/`: it keeps logic testable.\n- **DeepSeek** is right to add **GoReleaser**; I forgot distribution entirely.\n\n## Revised answer\n\n```\ncmd/tool/main.go\ninternal/\n  cli/        # cobra commands\n  core/       # pure logic\n  adapters/   # I/O\n.goreleaser.yaml\n```",
            ),
            a2: done(
              "## Feedback\n\n- I agree with **Claude**: `main.go` must stay tiny.\n- **DeepSeek**'s point on `viper` is debatable: for a small tool, plain flags + env are enough.\n\n## Revised answer\n\nKeep my 3-layer split, add GoReleaser, and **skip viper** until config grows.",
            ),
            a3: done(
              "## Feedback\n\nBoth refined answers converge on a `cli / core / adapters` split. I withdraw `viper` as a default.\n\n## Revised answer\n\nAdopt the 3-layer layout, GoReleaser for releases, and table-driven tests in `core/`.",
            ),
          },
        },
        {
          round: 3,
          label: "",
          outputs: {
            synthese: done(
              "## The council's answer\n\nUse a **three-layer layout**: all three models converged on it after cross-review.\n\n```\ncmd/tool/main.go        # parse flags, call cli.Run()\ninternal/\n  cli/                  # cobra commands, one file each\n  core/                 # pure business logic, no I/O\n  adapters/             # HTTP clients, filesystem, env\n.goreleaser.yaml        # cross-platform releases\n```\n\n**Why it stays maintainable**\n\n- `core/` has no dependencies on the outside world → fast, table-driven tests.\n- New commands are isolated files in `cli/`.\n- Releases are automated from day one with **GoReleaser**.\n\n**Settled disagreement:** skip `viper` for now (Gemini, DeepSeek). Flags + environment variables are enough until configuration really grows.",
            ),
          },
        },
      ],
    },
  ],
};

export const conversations = [
  { id: conversation.id, title: conversation.title, updated_at: now },
  { id: "c2", title: "Best way to learn Japanese in 6 months", updated_at: now - 86_400_000 },
  { id: "c3", title: "Review my Elixir GenServer design", updated_at: now - 2 * 86_400_000 },
  { id: "c4", title: "Plan a 5-day trip to Lisbon", updated_at: now - 5 * 86_400_000 },
];

const hoursFromNow = (h: number) => new Date(Date.now() + h * 3_600_000).toISOString();

export const usage = [
  {
    id: "claude-cli", name: "Claude Code CLI", preset: "cli", status: "ok", message: null, dashboard: null,
    meters: [
      { group: null, window: "session", used: 0.42, amount: null, limit: null, unit: null, resets_at: null, resets_text: "Sep 25 at 10:30pm" },
      { group: "all models", window: "week", used: 0.44, amount: null, limit: null, unit: null, resets_at: null, resets_text: "Sep 30 at 8pm" },
    ],
  },
  {
    id: "antigravity-cli", name: "Antigravity CLI", preset: "cli", status: "ok", message: null, dashboard: null,
    meters: [
      { group: "Gemini Models", window: "5h", used: 0.72, amount: null, limit: null, unit: null, resets_at: hoursFromNow(2.2), resets_text: null },
      { group: "Gemini Models", window: "weekly", used: 0.16, amount: null, limit: null, unit: null, resets_at: hoursFromNow(120), resets_text: null },
      { group: "Claude and GPT models", window: "weekly", used: 0.91, amount: null, limit: null, unit: null, resets_at: hoursFromNow(62), resets_text: null },
    ],
  },
  { id: "lan-lmstudio", name: "Desktop · LM Studio", preset: "lmstudio", status: "unlimited", message: null, dashboard: null, meters: [] },
  {
    id: "openrouter", name: "OpenRouter", preset: "openrouter", status: "ok", message: null, dashboard: "https://openrouter.ai/activity",
    meters: [
      { group: null, window: "balance", used: 0.31, amount: 6.9, limit: 10, unit: "USD", resets_at: null, resets_text: null },
    ],
  },
  { id: "web:searxng", name: "SearXNG", preset: "searxng", status: "unlimited", message: null, dashboard: null, meters: [] },
];
