use super::{Chunk, Sink};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Chemin complet d'une CLI : sous Windows, `claude` est un `claude.cmd` que
/// `Command::new("claude")` ne trouverait pas.
pub fn program(name: &str) -> std::ffi::OsString {
    which::which(name)
        .map(|p| p.into_os_string())
        .unwrap_or_else(|_| name.into())
}

/// Interprétation d'une ligne NDJSON émise par une CLI d'agent.
#[derive(Debug, PartialEq)]
pub enum Line {
    Delta(String),
    Final(Result<String, String>),
    Ignore,
}

/// Lance la CLI, streame ses deltas et renvoie la réponse finale.
/// Le processus est tué si la future est abandonnée (annulation).
pub async fn run(
    mut cmd: Command,
    stdin_data: Option<String>,
    parse: fn(&str) -> Line,
    on_delta: Sink<'_>,
) -> Result<String, String> {
    let program = cmd.as_std().get_program().to_string_lossy().into_owned();
    cmd.stdin(if stdin_data.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Impossible de lancer `{program}` : {e}"))?;

    if let (Some(data), Some(mut stdin)) = (stdin_data, child.stdin.take()) {
        tokio::spawn(async move {
            let _ = stdin.write_all(data.as_bytes()).await;
        });
    }

    let mut stderr = child.stderr.take().expect("stderr piped");
    let stderr_task = tokio::spawn(async move {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf).await;
        buf
    });

    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = BufReader::new(stdout).lines();
    let mut streamed = String::new();
    let mut final_result = None;

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| format!("Lecture de la sortie de `{program}` : {e}"))?
    {
        match parse(&line) {
            Line::Delta(text) => {
                streamed.push_str(&text);
                on_delta(Chunk::Text(&text));
            }
            Line::Final(result) => final_result = Some(result),
            Line::Ignore => {}
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Attente de `{program}` : {e}"))?;
    let stderr = stderr_task.await.unwrap_or_default();

    match final_result {
        // Une CLI peut « réussir » sans rien écrire (outil refusé, par exemple) :
        // c'est une erreur, avec l'explication qu'elle a laissée sur stderr.
        Some(Ok(text)) if text.trim().is_empty() && streamed.trim().is_empty() => {
            let detail = stderr.trim();
            Err(if detail.is_empty() {
                format!("`{program}` n'a produit aucune réponse")
            } else {
                format!("`{program}` : {detail}")
            })
        }
        Some(result) => result,
        None if status.success() && !streamed.is_empty() => Ok(streamed),
        None => {
            let detail = stderr.trim();
            Err(if detail.is_empty() {
                format!("`{program}` s'est arrêté sans réponse ({status})")
            } else {
                format!("`{program}` : {detail}")
            })
        }
    }
}
