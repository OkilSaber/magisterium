use crate::prompts;
use crate::providers::{self, Backend, Chunk, ExecMode, RunCtx};
use crate::tools::ModelTools;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub const SYNTH_KEY: &str = "synthese";

#[derive(Deserialize, Clone, Debug)]
pub struct AgentSpec {
    /// Identifiant unique dans la session (le même provider peut apparaître deux fois).
    pub key: String,
    pub label: String,
    /// Id du fournisseur (`claude-cli`, `antigravity-cli` ou un fournisseur configuré).
    pub provider: String,
    pub model: String,
    /// Niveau de réflexion (vide : réglage par défaut du fournisseur).
    #[serde(default)]
    pub effort: String,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Compare,
    Debate,
}

#[derive(Deserialize, Debug)]
pub struct RunConfig {
    pub prompt: String,
    pub agents: Vec<AgentSpec>,
    pub mode: Mode,
    pub rounds: u32,
    pub synthesizer: Option<AgentSpec>,
    pub workdir: String,
    pub exec_mode: ExecMode,
    /// Langue de l'interface, utilisée pour les consignes envoyées aux IA.
    #[serde(default = "default_lang")]
    pub lang: String,
    /// Donne les outils web aux modèles API et locaux.
    #[serde(default)]
    pub web: bool,
    /// Date du jour côté utilisateur (AAAA-MM-JJ), transmise aux modèles.
    #[serde(default)]
    pub today: String,
    /// Échanges précédents de la discussion, du plus ancien au plus récent.
    #[serde(default)]
    pub history: Vec<Exchange>,
}

/// Un tour déjà terminé : le message de l'utilisateur et la réponse du conseil
/// (la synthèse, ou à défaut les réponses finales de chaque IA).
#[derive(Deserialize, Clone, Debug)]
pub struct Exchange {
    pub prompt: String,
    pub answer: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunEvent {
    Phase {
        round: u32,
        label: String,
    },
    Start {
        agent: String,
        round: u32,
    },
    Delta {
        agent: String,
        round: u32,
        text: String,
    },
    /// Raisonnement du modèle, affiché à part de la réponse.
    Thinking {
        agent: String,
        round: u32,
        text: String,
    },
    /// Le modèle utilise un outil web : `detail` est la requête ou l'URL.
    Tool {
        agent: String,
        round: u32,
        name: String,
        detail: String,
    },
    Done {
        agent: String,
        round: u32,
        text: String,
    },
    Error {
        agent: String,
        round: u32,
        message: String,
    },
    Finished {
        cancelled: bool,
    },
}

pub type Emit<'a> = &'a (dyn Fn(RunEvent) + Send + Sync);

/// Déroule une session : tours en parallèle, revue croisée, puis synthèse.
fn default_lang() -> String {
    "fr".into()
}

/// Déroule une session. `backends` associe chaque id de fournisseur à la façon
/// de le joindre (résolu en amont, clés API comprises).
pub async fn execute(
    cfg: RunConfig,
    backends: &HashMap<String, Backend>,
    web: Option<&ModelTools>,
    emit: Emit<'_>,
) {
    let workdir = PathBuf::from(&cfg.workdir);
    let ctx = RunCtx {
        workdir: &workdir,
        mode: cfg.exec_mode,
        web,
    };
    let question = prompts::with_history(&cfg.lang, &cfg.history, &cfg.prompt);
    let rounds = match cfg.mode {
        Mode::Compare => 1,
        Mode::Debate => cfg.rounds.clamp(2, 5),
    };

    let mut previous: Vec<(AgentSpec, String)> = Vec::new();
    for round in 1..=rounds {
        emit(RunEvent::Phase {
            round,
            label: if round == 1 {
                "Réponses initiales".into()
            } else {
                format!("Revue croisée · tour {round}")
            },
        });

        let runs = cfg.agents.iter().map(|agent| {
            let prompt = if round == 1 {
                question.clone()
            } else {
                prompts::debate(&cfg.lang, &question, agent, &previous)
            };
            run_agent(agent, round, prompt, &ctx, backends, emit)
        });
        let results = join_all(runs).await;

        let answers: Vec<(AgentSpec, String)> = cfg
            .agents
            .iter()
            .cloned()
            .zip(results)
            .filter_map(|(agent, res)| res.map(|text| (agent, text)))
            .collect();
        if answers.is_empty() {
            return;
        }
        previous = answers;
    }

    if let Some(synth) = &cfg.synthesizer {
        let round = rounds + 1;
        emit(RunEvent::Phase {
            round,
            label: "Synthèse".into(),
        });
        let spec = AgentSpec {
            key: SYNTH_KEY.into(),
            ..synth.clone()
        };
        let prompt = prompts::synthesis(&cfg.lang, &question, &previous);
        run_agent(&spec, round, prompt, &ctx, backends, emit).await;
    }
}

/// Délais avant chaque nouvel essai quand le service est surchargé.
const RETRY_DELAYS: [u64; 2] = [3, 10];

async fn run_agent(
    agent: &AgentSpec,
    round: u32,
    prompt: String,
    ctx: &RunCtx<'_>,
    backends: &HashMap<String, Backend>,
    emit: Emit<'_>,
) -> Option<String> {
    let Some(backend) = backends.get(&agent.provider) else {
        emit(RunEvent::Error {
            agent: agent.key.clone(),
            round,
            message: format!("Fournisseur introuvable : {}", agent.provider),
        });
        return None;
    };
    let on_delta = |chunk: Chunk<'_>| {
        let agent = agent.key.clone();
        emit(match chunk {
            Chunk::Text(t) => RunEvent::Delta {
                agent,
                round,
                text: t.to_string(),
            },
            Chunk::Thinking(t) => RunEvent::Thinking {
                agent,
                round,
                text: t.to_string(),
            },
            Chunk::Tool { name, detail } => RunEvent::Tool {
                agent,
                round,
                name: name.to_string(),
                detail: detail.to_string(),
            },
        })
    };

    let mut attempt = 0;
    loop {
        // `Start` remet la colonne à zéro côté UI, y compris entre deux essais.
        emit(RunEvent::Start {
            agent: agent.key.clone(),
            round,
        });
        match providers::run(
            backend,
            &agent.model,
            &agent.effort,
            &prompt,
            ctx,
            &on_delta,
        )
        .await
        {
            Ok(text) => {
                emit(RunEvent::Done {
                    agent: agent.key.clone(),
                    round,
                    text: text.clone(),
                });
                return Some(text);
            }
            Err(message) if is_transient(&message) && attempt < RETRY_DELAYS.len() => {
                let delay = RETRY_DELAYS[attempt];
                attempt += 1;
                emit(RunEvent::Delta {
                    agent: agent.key.clone(),
                    round,
                    text: format!(
                        "_Service surchargé, nouvel essai dans {delay} s ({attempt}/{})…_",
                        RETRY_DELAYS.len()
                    ),
                });
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            }
            Err(message) => {
                emit(RunEvent::Error {
                    agent: agent.key.clone(),
                    round,
                    message,
                });
                return None;
            }
        }
    }
}

/// Erreurs de surcharge côté fournisseur, qui valent la peine d'être réessayées.
fn is_transient(message: &str) -> bool {
    let m = message.to_lowercase();
    [
        "503",
        "429",
        "unavailable",
        "overloaded",
        "rate limit",
        "resource_exhausted",
    ]
    .iter()
    .any(|needle| m.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn cli_backends() -> HashMap<String, Backend> {
        [providers::CLAUDE_CLI, providers::ANTIGRAVITY_CLI]
            .into_iter()
            .map(|id| (id.to_string(), providers::cli_backend(id).unwrap()))
            .collect()
    }

    #[test]
    fn detects_transient_errors() {
        assert!(is_transient(
            "failed to get load code assist response: UNAVAILABLE (code 503): The service is currently unavailable."
        ));
        assert!(is_transient("HTTP 429 Too Many Requests : rate limit"));
        assert!(!is_transient("`claude` introuvable dans le PATH"));
        assert!(!is_transient("Failed to load model \"bonsai\""));
    }

    /// Session réelle Claude + Antigravity, débat en 2 tours puis synthèse.
    /// `cargo test -- --ignored live_debate --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_debate() {
        crate::env::init_path();
        let dir = std::env::temp_dir().join("magisterium-live");
        std::fs::create_dir_all(&dir).unwrap();
        let agent = |key: &str, provider, model: &str| AgentSpec {
            key: key.into(),
            label: key.into(),
            provider,
            model: model.into(),
            effort: String::new(),
        };
        let cfg = RunConfig {
            prompt: "En une phrase : vaut-il mieux des tabs ou des espaces ?".into(),
            agents: vec![
                agent("claude", providers::CLAUDE_CLI.to_string(), "haiku"),
                agent(
                    "gemini",
                    providers::ANTIGRAVITY_CLI.to_string(),
                    "gemini-3.8-flash-low",
                ),
            ],
            mode: Mode::Debate,
            rounds: 2,
            synthesizer: Some(agent("synth", providers::CLAUDE_CLI.to_string(), "haiku")),
            workdir: dir.to_string_lossy().into(),
            exec_mode: ExecMode::Edit,
            lang: "fr".into(),
            web: false,
            today: String::new(),
            history: Vec::new(),
        };
        let events = Mutex::new(Vec::new());
        execute(cfg, &cli_backends(), None, &|e| {
            events.lock().unwrap().push(e)
        })
        .await;

        let events = events.into_inner().unwrap();
        for e in &events {
            match e {
                RunEvent::Delta { .. } => {}
                other => println!("{other:?}"),
            }
        }
        let done: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                RunEvent::Done { agent, round, .. } => Some((agent.as_str(), *round)),
                _ => None,
            })
            .collect();
        assert!(done.contains(&("claude", 2)));
        assert!(done.contains(&("gemini", 2)));
        assert!(done.contains(&(SYNTH_KEY, 3)));
    }

    /// Abandonner la session (comme le fait `cancel_run`) doit tuer les CLI.
    #[tokio::test]
    #[ignore]
    async fn live_cancel_kills_children() {
        crate::env::init_path();
        let cfg = RunConfig {
            prompt: "Écris un essai de 3000 mots sur l'histoire de Rust.".into(),
            agents: vec![
                AgentSpec {
                    key: "c".into(),
                    label: "c".into(),
                    provider: providers::CLAUDE_CLI.to_string(),
                    model: "haiku".into(),
                    effort: String::new(),
                },
                AgentSpec {
                    key: "g".into(),
                    label: "g".into(),
                    provider: providers::ANTIGRAVITY_CLI.to_string(),
                    model: "gemini-3.8-flash-low".into(),
                    effort: String::new(),
                },
            ],
            mode: Mode::Compare,
            rounds: 1,
            synthesizer: None,
            workdir: std::env::temp_dir().to_string_lossy().into(),
            exec_mode: ExecMode::Edit,
            lang: "fr".into(),
            web: false,
            today: String::new(),
            history: Vec::new(),
        };
        let emit = |_e: RunEvent| {};
        let finished = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            execute(cfg, &cli_backends(), None, &emit),
        )
        .await;
        assert!(
            finished.is_err(),
            "la session aurait dû être encore en cours"
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // Seulement nos propres enfants : d'autres sessions Claude peuvent tourner.
        let left = std::process::Command::new("pgrep")
            .args(["-P", &std::process::id().to_string()])
            .output()
            .unwrap();
        assert!(
            left.stdout.is_empty(),
            "processus restants : {}",
            String::from_utf8_lossy(&left.stdout)
        );
    }

    /// Le message de suite doit arriver aux IA avec l'historique de la discussion.
    #[tokio::test]
    #[ignore]
    async fn live_follow_up_uses_history() {
        crate::env::init_path();
        let agent = |key: &str, provider, model: &str| AgentSpec {
            key: key.into(),
            label: key.into(),
            provider,
            model: model.into(),
            effort: String::new(),
        };
        let cfg = RunConfig {
            prompt: "Rappelle-moi mon prénom, en un seul mot.".into(),
            agents: vec![
                agent("claude", providers::CLAUDE_CLI.to_string(), "haiku"),
                agent(
                    "gemini",
                    providers::ANTIGRAVITY_CLI.to_string(),
                    "gemini-3.8-flash-low",
                ),
            ],
            mode: Mode::Compare,
            rounds: 1,
            synthesizer: None,
            workdir: std::env::temp_dir().to_string_lossy().into(),
            exec_mode: ExecMode::Plan,
            lang: "fr".into(),
            web: false,
            today: String::new(),
            history: vec![Exchange {
                prompt: "Je m'appelle Zorblax.".into(),
                answer: "Enchanté, Zorblax !".into(),
            }],
        };
        // Vérifie au passage que les CLI acceptent le niveau de réflexion et le mode Plan.
        let mut cfg = cfg;
        for a in &mut cfg.agents {
            a.effort = "low".into();
        }
        let events = Mutex::new(Vec::new());
        execute(cfg, &cli_backends(), None, &|e| {
            events.lock().unwrap().push(e)
        })
        .await;
        let answers: Vec<String> = events
            .into_inner()
            .unwrap()
            .into_iter()
            .filter_map(|e| match e {
                RunEvent::Done { agent, text, .. } => Some(format!("{agent}: {text}")),
                _ => None,
            })
            .collect();
        println!("{answers:#?}");
        assert_eq!(answers.len(), 2);
        assert!(answers.iter().all(|a| a.contains("Zorblax")));
    }

    /// Un serveur local compatible OpenAI (LM Studio sur :1234 avec Rnj-1 chargé).
    #[tokio::test]
    #[ignore]
    async fn live_local_openai_server() {
        let backends: HashMap<String, Backend> = [(
            "local".to_string(),
            Backend::Api {
                kind: crate::config::ApiKind::Openai,
                base_url: "http://localhost:1234/v1".into(),
                key: None,
            },
        )]
        .into();
        let cfg = RunConfig {
            prompt: "Réponds uniquement par le mot : bonjour".into(),
            agents: vec![AgentSpec {
                key: "rnj".into(),
                label: "Rnj-1".into(),
                provider: "local".into(),
                model: "essentialai/rnj-1".into(),
                effort: String::new(),
            }],
            mode: Mode::Compare,
            rounds: 1,
            synthesizer: None,
            workdir: std::env::temp_dir().to_string_lossy().into(),
            exec_mode: ExecMode::Plan,
            lang: "fr".into(),
            web: false,
            today: String::new(),
            history: Vec::new(),
        };
        let events = Mutex::new(Vec::new());
        execute(cfg, &backends, None, &|e| events.lock().unwrap().push(e)).await;
        let events = events.into_inner().unwrap();
        let deltas = events
            .iter()
            .filter(|e| matches!(e, RunEvent::Delta { .. }))
            .count();
        let done = events.iter().find_map(|e| match e {
            RunEvent::Done { text, .. } => Some(text.clone()),
            _ => None,
        });
        println!("deltas={deltas} done={done:?}");
        assert!(deltas > 0);
        assert!(done.unwrap().to_lowercase().contains("bonjour"));
    }
}
