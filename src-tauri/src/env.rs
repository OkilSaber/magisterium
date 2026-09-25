use std::process::{Command, Stdio};

const MARKER: &str = "__MAGISTERIUM_PATH__";

/// Une app lancée depuis le Finder n'hérite pas du PATH du shell (nvm, asdf,
/// ~/.local/bin…) : on le récupère depuis un zsh interactif pour trouver
/// `claude`, `agy`, `lms`, `ollama`.
pub fn init_path() {
    let home = std::env::var("HOME").unwrap_or_default();

    let shell_path = Command::new("/bin/zsh")
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

    for extra in [
        format!("{home}/.local/bin"),
        format!("{home}/.lmstudio/bin"),
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
    ] {
        if !parts.contains(&extra) {
            parts.push(extra);
        }
    }

    std::env::set_var("PATH", parts.join(":"));
}
