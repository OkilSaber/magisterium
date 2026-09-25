use super::cli::{self, Line};
use super::{ExecMode, ModelInfo, ProviderInfo, RunCtx, Sink, CLAUDE_CLI};
use serde_json::Value;
use tokio::process::Command;

pub async fn detect() -> Option<ProviderInfo> {
    which::which("claude").ok()?;
    Some(ProviderInfo {
        id: CLAUDE_CLI.into(),
        name: "Claude Code CLI".into(),
        preset: "cli".into(),
        available: true,
        status: None,
        models: vec![
            ModelInfo::new("", "Par défaut"),
            ModelInfo::new("opus", "Opus"),
            ModelInfo::new("sonnet", "Sonnet"),
            ModelInfo::new("haiku", "Haiku"),
        ],
        uses_tools: true,
        efforts: vec!["low", "medium", "high", "xhigh", "max"],
    })
}

fn command(model: &str, effort: &str, ctx: &RunCtx<'_>) -> Command {
    let mut cmd = Command::new(cli::program("claude"));
    cmd.args([
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
    ]);
    if !model.is_empty() {
        cmd.args(["--model", model]);
    }
    if !effort.is_empty() {
        cmd.args(["--effort", effort]);
    }
    match ctx.mode {
        ExecMode::Plan => cmd.args(["--permission-mode", "plan"]),
        ExecMode::Edit => cmd.args(["--permission-mode", "acceptEdits"]),
        ExecMode::Auto => cmd.args(["--permission-mode", "auto"]),
        ExecMode::Full => cmd.arg("--dangerously-skip-permissions"),
    };
    // Évite que la CLI se croie imbriquée dans une session Claude Code parente.
    cmd.env_remove("CLAUDECODE").current_dir(ctx.workdir);
    cmd
}

pub async fn run(
    model: &str,
    effort: &str,
    prompt: &str,
    ctx: &RunCtx<'_>,
    on_delta: Sink<'_>,
) -> Result<String, String> {
    cli::run(
        command(model, effort, ctx),
        Some(prompt.to_string()),
        parse_line,
        on_delta,
    )
    .await
}

pub fn parse_line(line: &str) -> Line {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Line::Ignore;
    };
    match v["type"].as_str() {
        Some("stream_event") => {
            let event = &v["event"];
            match (event["type"].as_str(), event["delta"]["type"].as_str()) {
                (Some("content_block_delta"), Some("text_delta")) => {
                    Line::Delta(event["delta"]["text"].as_str().unwrap_or_default().into())
                }
                _ => Line::Ignore,
            }
        }
        Some("result") => {
            let text = v["result"].as_str().unwrap_or_default().to_string();
            if v["is_error"].as_bool() == Some(true) {
                let subtype = v["subtype"].as_str().unwrap_or("erreur");
                Line::Final(Err(if text.is_empty() {
                    subtype.to_string()
                } else {
                    text
                }))
            } else {
                Line::Final(Ok(text))
            }
        }
        _ => Line::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixture() {
        let fixture = include_str!("../../tests/fixtures/claude.ndjson");
        let mut deltas = String::new();
        let mut result = None;
        for line in fixture.lines() {
            match parse_line(line) {
                Line::Delta(t) => deltas.push_str(&t),
                Line::Final(r) => result = Some(r),
                Line::Ignore => {}
            }
        }
        assert_eq!(deltas, "Bonjour, ça va bien.");
        assert_eq!(result, Some(Ok("Bonjour, ça va bien.".to_string())));
    }

    #[test]
    fn parse_error_result() {
        let line = r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#;
        assert_eq!(parse_line(line), Line::Final(Err("error_max_turns".into())));
    }
}
