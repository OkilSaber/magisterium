use std::process::{Command, Stdio};

const MARKER: &str = "__MAGISTERIUM_PATH__";

/// Sous Windows, une app hérite du PATH système : rien à faire.
#[cfg(windows)]
pub fn init_path() {}

/// Une app lancée depuis le Finder ou le menu du bureau n'hérite pas du PATH du
/// shell (nvm, asdf, ~/.local/bin…) : on le récupère depuis un shell interactif
/// pour trouver `claude`, `agy`, `lms`, `ollama`.
#[cfg(unix)]
pub fn init_path() {
    let home = std::env::var("HOME").unwrap_or_default();
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| std::path::Path::new(s).exists())
        .unwrap_or_else(|| {
            if std::path::Path::new("/bin/zsh").exists() {
                "/bin/zsh".into()
            } else {
                "/bin/sh".into()
            }
        });

    let shell_path = Command::new(&shell)
        .args(["-ilc", &format!("printf '{MARKER}%s{MARKER}' \"$PATH\"")])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .split(MARKER)
                .nth(1)
                .map(str::to_string)
        });

    let base = shell_path
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();

    let mut parts: Vec<String> = base
        .split(':')
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();

    let mut extras = vec![
        format!("{home}/.local/bin"),
        format!("{home}/.lmstudio/bin"),
    ];
    if cfg!(target_os = "macos") {
        extras.push("/opt/homebrew/bin".into());
    }
    extras.push("/usr/local/bin".into());
    for extra in extras {
        if !parts.contains(&extra) {
            parts.push(extra);
        }
    }

    std::env::set_var("PATH", parts.join(":"));
}
