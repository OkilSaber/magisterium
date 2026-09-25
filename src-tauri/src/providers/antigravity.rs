use super::cli::{self, Line};
use super::{ExecMode, ModelInfo, ProviderInfo, RunCtx, Sink, ANTIGRAVITY_CLI};
use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;

pub async fn detect() -> Option<ProviderInfo> {
    which::which("agy").ok()?;
    let mut info = ProviderInfo {
        id: ANTIGRAVITY_CLI.into(),
        name: "Antigravity CLI".into(),
        preset: "cli".into(),
        available: true,
        status: None,
        models: vec![ModelInfo::new("", "Par défaut")],
        uses_tools: true,
        // Le niveau de réflexion est déjà dans le modèle (« Gemini 3.8 Flash (High) »…).
        efforts: Vec::new(),
    };
    match list_models().await {
        Ok(models) => info.models.extend(models),
        Err(e) => info.status = Some(format!("Liste des modèles indisponible : {e}")),
    }
    Some(info)
}

async fn list_models() -> Result<Vec<ModelInfo>, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(20),
        Command::new(cli::program("agy"))
            .arg("models")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "délai dépassé".to_string())?
    .map_err(|e| e.to_string())?;
    Ok(parse_models(&String::from_utf8_lossy(&output.stdout)))
}

/// `agy models` affiche une ligne `id<TAB>libellé` par modèle.
fn parse_models(stdout: &str) -> Vec<ModelInfo> {
    stdout
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(id, label)| ModelInfo::new(id.trim(), label.trim()))
        .collect()
}

const EDIT_MODE_HINT: &str = "[Environment note: shell commands (run_command) are not \
    permitted in this session and will be denied. To explore or change the workspace, use \
    your file tools instead (list_dir, view_file, grep_search, file editing tools).]";

fn command(model: &str, prompt: &str, ctx: &RunCtx<'_>) -> Command {
    let mut cmd = Command::new(cli::program("agy"));
    cmd.args(["--output-format", "stream-json"]);
    if !model.is_empty() {
        cmd.args(["--model", model]);
    }
    match ctx.mode {
        ExecMode::Plan => cmd.args(["--mode", "plan"]),
        // Antigravity n'a pas de mode auto : on reste sur l'édition acceptée.
        ExecMode::Edit | ExecMode::Auto => cmd.args(["--mode", "accept-edits"]),
        ExecMode::Full => cmd.arg("--dangerously-skip-permissions"),
    };
    // En mode édition, les commandes shell sont refusées d'office (personne ne peut
    // les valider) et Antigravity abandonne : on l'oriente vers ses outils de fichiers.
    let prompt = match ctx.mode {
        ExecMode::Edit | ExecMode::Auto => format!("{EDIT_MODE_HINT}\n\n{prompt}"),
        _ => prompt.to_string(),
    };
    // `agy` ne lit pas le prompt sur stdin : il doit être collé au flag.
    cmd.arg(format!("-p={prompt}")).current_dir(ctx.workdir);
    cmd
}

pub async fn run(
    model: &str,
    prompt: &str,
    ctx: &RunCtx<'_>,
    on_delta: Sink<'_>,
) -> Result<String, String> {
    cli::run(command(model, prompt, ctx), None, parse_line, on_delta).await
}

pub fn parse_line(line: &str) -> Line {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Line::Ignore;
    };
    match v["event"].as_str() {
        Some("step_update") => {
            let step = &v["step_update"];
            match (step["step_type"].as_str(), step["text_delta"].as_str()) {
                (Some("agent_response"), Some(text)) => Line::Delta(text.into()),
                _ => Line::Ignore,
            }
        }
        Some("result") => {
            let result = &v["result"];
            match result["status"].as_str() {
                Some("SUCCESS") => {
                    Line::Final(Ok(result["response"].as_str().unwrap_or_default().into()))
                }
                status => {
                    let detail = result["error"]
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| format!("statut {}", status.unwrap_or("inconnu")));
                    Line::Final(Err(detail))
                }
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
        let fixture = include_str!("../../tests/fixtures/agy.ndjson");
        let mut deltas = String::new();
        let mut result = None;
        for line in fixture.lines() {
            match parse_line(line) {
                Line::Delta(t) => deltas.push_str(&t),
                Line::Final(r) => result = Some(r),
                Line::Ignore => {}
            }
        }
        assert_eq!(deltas, "Bonjour à tous.\n");
        assert_eq!(result, Some(Ok("Bonjour à tous.\n".to_string())));
    }

    #[test]
    fn parse_models_output() {
        let out = "Fetching available models...\ngemini-3.1-pro-high\tGemini 3.1 Pro (High)\n";
        let models = parse_models(out);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gemini-3.1-pro-high");
        assert_eq!(models[0].label, "Gemini 3.1 Pro (High)");
    }
}
