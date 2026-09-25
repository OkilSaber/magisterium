//! SearXNG embarqué : installé à la demande dans le dossier de l'app, sans rien
//! exiger de la machine. L'app télécharge `uv`, un Python autonome et les
//! sources de SearXNG, puis lance le serveur sur 127.0.0.1 tant qu'il est activé.

use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

// Versions figées : une installation est reproductible d'une machine à l'autre.
const UV_VERSION: &str = "0.12.19";
const PYTHON_BUILD: &str = "20260924";
const PYTHON_VERSION: &str = "3.12.14";
const SEARXNG_SHA: &str = "d8ae3abd59b6cb3970e3380e484c7d4cd8529619";

pub const STEPS: [&str; 6] = ["uv", "python", "searxng", "deps", "config", "verify"];

/// Emplacements, tous sous `<données de l'app>/searxng/`.
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    fn uv(&self) -> PathBuf {
        self.root.join("bin/uv")
    }
    fn python(&self) -> PathBuf {
        self.root.join("python/bin/python3")
    }
    fn src(&self) -> PathBuf {
        self.root.join("src")
    }
    fn venv_python(&self) -> PathBuf {
        self.root.join("venv/bin/python")
    }
    fn settings(&self) -> PathBuf {
        self.root.join("settings.yml")
    }
    fn marker(&self) -> PathBuf {
        self.root.join("installed.json")
    }
    fn log(&self) -> PathBuf {
        self.root.join("searxng.log")
    }
    pub fn installed(&self) -> bool {
        std::fs::read_to_string(self.marker())
            .map(|m| m.contains(SEARXNG_SHA))
            .unwrap_or(false)
    }
    pub fn size_bytes(&self) -> u64 {
        dir_size(&self.root)
    }
    /// Supprime toute l'installation.
    pub fn remove(&self) -> Result<(), String> {
        match std::fs::remove_dir_all(&self.root) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

/// Avancement d'une étape, envoyé à l'interface.
#[derive(Serialize, Clone, Debug)]
pub struct Progress {
    pub step: &'static str,
    pub index: usize,
    pub total: usize,
    /// « running », « done », « skipped » ou « error ».
    pub state: &'static str,
    pub bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub message: Option<String>,
}

impl Progress {
    fn new(index: usize, state: &'static str) -> Self {
        Self {
            step: STEPS[index],
            index,
            total: STEPS.len(),
            state,
            bytes: None,
            total_bytes: None,
            message: None,
        }
    }
}

pub type OnProgress<'a> = &'a (dyn Fn(Progress) + Send + Sync);

fn arch() -> &'static str {
    if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "aarch64"
    }
}

fn uv_url() -> String {
    format!(
        "https://github.com/astral-sh/uv/releases/download/{UV_VERSION}/uv-{}-apple-darwin.tar.gz",
        arch()
    )
}

fn python_url() -> String {
    format!(
        "https://github.com/astral-sh/python-build-standalone/releases/download/{PYTHON_BUILD}/\
         cpython-{PYTHON_VERSION}%2B{PYTHON_BUILD}-{}-apple-darwin-install_only.tar.gz",
        arch()
    )
}

fn searxng_url() -> String {
    format!("https://github.com/searxng/searxng/archive/{SEARXNG_SHA}.tar.gz")
}

/// Installe (ou termine d'installer) SearXNG. Chaque étape déjà faite est sautée.
pub async fn install(paths: &Paths, on: OnProgress<'_>) -> Result<(), String> {
    std::fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
    let result = run_steps(paths, on).await;
    // Le cache d'uv et les archives ne servent plus une fois installé.
    let _ = std::fs::remove_dir_all(paths.root.join("cache"));
    let _ = std::fs::remove_dir_all(paths.root.join("downloads"));
    result
}

async fn run_steps(paths: &Paths, on: OnProgress<'_>) -> Result<(), String> {
    let bin = paths.root.join("bin");
    let python_dir = paths.root.join("python");
    step(
        0,
        paths.uv().exists(),
        on,
        fetch_archive(0, &uv_url(), &bin, on),
    )
    .await?;
    step(
        1,
        paths.python().exists(),
        on,
        fetch_archive(1, &python_url(), &python_dir, on),
    )
    .await?;
    // Les sources portent la version extraite : une installation interrompue ne
    // retélécharge pas SearXNG, une nouvelle version épinglée si.
    let src_marker = paths.src().join(".magisterium-sha");
    let src_ok = std::fs::read_to_string(&src_marker).is_ok_and(|s| s == SEARXNG_SHA);
    step(2, src_ok, on, async {
        let _ = std::fs::remove_dir_all(paths.src());
        fetch_archive(2, &searxng_url(), &paths.src(), on).await?;
        std::fs::write(&src_marker, SEARXNG_SHA).map_err(|e| e.to_string())
    })
    .await?;
    step(3, paths.installed(), on, install_deps(paths, on)).await?;
    step(4, paths.settings().exists(), on, write_settings(paths)).await?;
    step(5, false, on, verify(paths)).await?;

    let marker =
        serde_json::json!({ "uv": UV_VERSION, "python": PYTHON_VERSION, "searxng": SEARXNG_SHA });
    std::fs::write(paths.marker(), marker.to_string()).map_err(|e| e.to_string())
}

/// Exécute une étape en signalant son début et sa fin, ou la saute si elle est déjà faite.
async fn step(
    index: usize,
    done: bool,
    on: OnProgress<'_>,
    work: impl std::future::Future<Output = Result<(), String>>,
) -> Result<(), String> {
    if done {
        on(Progress::new(index, "skipped"));
        return Ok(());
    }
    on(Progress::new(index, "running"));
    match work.await {
        Ok(()) => {
            on(Progress::new(index, "done"));
            Ok(())
        }
        Err(e) => {
            on(Progress {
                message: Some(e.clone()),
                ..Progress::new(index, "error")
            });
            Err(e)
        }
    }
}

/// Télécharge une archive `.tar.gz` en signalant les octets reçus, puis l'extrait
/// dans `dest` en retirant le premier dossier de l'archive.
async fn fetch_archive(
    index: usize,
    url: &str,
    dest: &Path,
    on: OnProgress<'_>,
) -> Result<(), String> {
    let downloads = dest.parent().unwrap_or(dest).join("downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
    let file_path = downloads.join(format!("step-{index}.tar.gz"));

    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Téléchargement impossible : {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Téléchargement impossible : HTTP {}",
            resp.status()
        ));
    }
    let total = resp.content_length().filter(|n| *n > 0);
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut received: u64 = 0;
    let mut last = Instant::now() - Duration::from_secs(1);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Téléchargement interrompu : {e}"))?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        received += chunk.len() as u64;
        // Une dizaine de mises à jour par seconde suffit à l'œil.
        if last.elapsed() >= Duration::from_millis(100) {
            last = Instant::now();
            on(Progress {
                bytes: Some(received),
                total_bytes: total,
                ..Progress::new(index, "running")
            });
        }
    }
    drop(file);

    let (archive, dest) = (file_path.clone(), dest.to_path_buf());
    tokio::task::spawn_blocking(move || extract(&archive, &dest))
        .await
        .map_err(|e| e.to_string())??;
    let _ = std::fs::remove_file(file_path);
    Ok(())
}

fn extract(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        // Retire le dossier racine de l'archive (« uv-aarch64-apple-darwin/uv » → « uv »).
        let stripped: PathBuf = path.components().skip(1).collect();
        if stripped.as_os_str().is_empty() {
            continue;
        }
        // Un lien peut précéder son dossier dans l'archive : on crée le parent d'abord.
        let target = dest.join(stripped);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        entry
            .unpack(&target)
            .map_err(|e| format!("Extraction impossible : {e}"))?;
    }
    Ok(())
}

/// Environnement d'uv confiné au dossier de l'app (ni cache ni Python de l'utilisateur).
fn uv(paths: &Paths) -> Command {
    let mut cmd = Command::new(paths.uv());
    cmd.env("UV_CACHE_DIR", paths.root.join("cache"))
        .env("UV_PYTHON_INSTALL_DIR", paths.root.join("uv-python"))
        .env("UV_NO_CONFIG", "1")
        .env("UV_PYTHON_DOWNLOADS", "never")
        .env("VIRTUAL_ENV", paths.root.join("venv"))
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    cmd
}

async fn install_deps(paths: &Paths, on: OnProgress<'_>) -> Result<(), String> {
    let venv_py = paths.venv_python();
    let venv_py = venv_py.to_string_lossy();
    let python = paths.python();
    let python = python.to_string_lossy();
    let src = paths.src();
    let src = src.to_string_lossy();
    let steps: [Vec<&str>; 3] = [
        vec!["venv", "--clear", "--python", &python, "venv"],
        vec![
            "pip",
            "install",
            "--python",
            &venv_py,
            "-U",
            "pip",
            "setuptools",
            "wheel",
            "pyyaml",
            "msgspec",
            "typing-extensions",
            "pybind11",
        ],
        vec![
            "pip",
            "install",
            "--python",
            &venv_py,
            "--no-build-isolation",
            "-e",
            &src,
        ],
    ];
    for args in steps {
        let mut child = uv(paths)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("uv : {e}"))?;
        // uv écrit sa progression sur stderr : on relaie la dernière ligne.
        let mut lines = BufReader::new(child.stderr.take().expect("stderr")).lines();
        let mut last = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if !line.is_empty() {
                on(Progress {
                    message: Some(line.clone()),
                    ..Progress::new(3, "running")
                });
                last = line;
            }
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Installation des dépendances échouée : {last}"));
        }
    }
    Ok(())
}

async fn write_settings(paths: &Paths) -> Result<(), String> {
    std::fs::write(
        paths.settings(),
        settings_yaml(&uuid::Uuid::new_v4().simple().to_string()),
    )
    .map_err(|e| e.to_string())
}

/// Configuration minimale : écoute locale, format JSON, pas de limiteur anti-robot
/// (il n'y a qu'un seul client, l'app elle-même).
fn settings_yaml(secret: &str) -> String {
    format!(
        "use_default_settings: true\n\
         general:\n  instance_name: \"Magisterium\"\n\
         server:\n  secret_key: \"{secret}\"\n  bind_address: \"127.0.0.1\"\n  port: 8931\n  \
         limiter: false\n  image_proxy: false\n\
         search:\n  formats: [html, json]\n"
    )
}

async fn verify(paths: &Paths) -> Result<(), String> {
    let server = Server::start(paths).await?;
    let hits = crate::tools::search(&server.engine(), "SearXNG").await;
    server.stop().await;
    hits.map(|_| ())
}

/// Serveur SearXNG en cours d'exécution.
pub struct Server {
    child: Child,
    pub port: u16,
}

impl Server {
    pub async fn start(paths: &Paths) -> Result<Self, String> {
        let port = free_port()?;
        let log = std::fs::File::create(paths.log()).map_err(|e| e.to_string())?;
        let mut child = Command::new(paths.venv_python())
            .args(["-m", "searx.webapp"])
            .env("SEARXNG_SETTINGS_PATH", paths.settings())
            .env("SEARXNG_PORT", port.to_string())
            .env("SEARXNG_BIND_ADDRESS", "127.0.0.1")
            .current_dir(&paths.root)
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Démarrage de SearXNG impossible : {e}"))?;

        let url = format!("http://127.0.0.1:{port}/healthz");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!(
                    "SearXNG s'est arrêté ({status}) : {}",
                    log_tail(paths)
                ));
            }
            let ok = reqwest::Client::new()
                .get(&url)
                .timeout(Duration::from_secs(1))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success());
            if ok {
                return Ok(Self { child, port });
            }
            if Instant::now() > deadline {
                let _ = child.kill().await;
                return Err(format!(
                    "SearXNG ne répond pas après 30 s : {}",
                    log_tail(paths)
                ));
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    pub fn engine(&self) -> crate::tools::SearchEngine {
        crate::tools::SearchEngine::Searxng {
            url: format!("http://127.0.0.1:{}", self.port),
        }
    }

    pub async fn stop(mut self) {
        let _ = self.child.kill().await;
    }

    /// Arrêt immédiat, sans attendre (fermeture de l'app).
    pub fn kill_now(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn free_port() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| e.to_string())
}

fn log_tail(paths: &Paths) -> String {
    let log = std::fs::read_to_string(paths.log()).unwrap_or_default();
    let lines: Vec<&str> = log.lines().rev().take(3).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// État partagé : serveur lancé et avancement, lisibles par l'interface.
#[derive(Default)]
pub struct SearxngState {
    pub server: Mutex<Option<Server>>,
    pub installing: Mutex<bool>,
    pub last_error: Mutex<Option<String>>,
    pub starting: Mutex<bool>,
}

impl SearxngState {
    pub fn port(&self) -> Option<u16> {
        self.server.lock().unwrap().as_ref().map(|s| s.port)
    }

    pub fn kill_now(&self) {
        if let Some(server) = self.server.lock().unwrap().as_mut() {
            server.kill_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_enable_json_and_local_binding() {
        let yaml = settings_yaml("abc");
        assert!(yaml.contains("secret_key: \"abc\""));
        assert!(yaml.contains("bind_address: \"127.0.0.1\""));
        assert!(yaml.contains("formats: [html, json]"));
        assert!(yaml.contains("limiter: false"));
    }

    #[test]
    fn download_urls_match_the_architecture() {
        assert!(uv_url().ends_with(&format!("uv-{}-apple-darwin.tar.gz", arch())));
        assert!(python_url().contains("cpython-3.12.14%2B20260924"));
        assert!(searxng_url().ends_with(&format!("{SEARXNG_SHA}.tar.gz")));
    }

    /// Installation complète dans un dossier temporaire, avec les vrais téléchargements.
    /// `cargo test live_builtin_searxng -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_builtin_searxng() {
        let root = std::env::temp_dir().join("magisterium-searxng-test");
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::new(root.clone());
        let events = Mutex::new(Vec::<Progress>::new());
        install(&paths, &|p| events.lock().unwrap().push(p))
            .await
            .unwrap();

        let events = events.into_inner().unwrap();
        for s in 0..STEPS.len() {
            assert!(
                events.iter().any(|p| p.index == s && p.state == "done"),
                "étape {s} pas faite"
            );
        }
        let max_bytes = events
            .iter()
            .filter(|p| p.index == 1)
            .filter_map(|p| p.bytes)
            .max();
        println!("octets Python reçus : {max_bytes:?}");
        assert!(max_bytes.unwrap_or(0) > 1_000_000);

        // Le Python utilisé est bien celui de l'app, pas celui de la machine.
        let cfg = std::fs::read_to_string(root.join("venv/pyvenv.cfg")).unwrap();
        assert!(
            cfg.contains(&root.join("python").to_string_lossy().to_string()),
            "{cfg}"
        );

        let server = Server::start(&paths).await.unwrap();
        let hits = crate::tools::search(&server.engine(), "Louis de Funès")
            .await
            .unwrap();
        println!(
            "{} résultats, taille installée {} Mo",
            hits.len(),
            paths.size_bytes() / 1_000_000
        );
        server.stop().await;
        assert!(!hits.is_empty());

        // Deuxième passage : tout est sauté sauf la vérification.
        let again = Mutex::new(Vec::<Progress>::new());
        install(&paths, &|p| again.lock().unwrap().push(p))
            .await
            .unwrap();
        let again = again.into_inner().unwrap();
        assert!((0..5).all(|s| again.iter().any(|p| p.index == s && p.state == "skipped")));
    }
}
