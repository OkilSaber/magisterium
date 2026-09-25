//! Discussions sauvegardées : un fichier JSON par discussion dans le dossier
//! de données de l'app. Le contenu est construit par le front ; ici on ne lit
//! que les champs utiles à la liste.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
}

fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("conversations");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// L'id devient un nom de fichier : on refuse tout ce qui pourrait sortir du dossier.
fn file(dir: &Path, id: &str) -> Result<PathBuf, String> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valid {
        return Err(format!("Identifiant de discussion invalide : {id}"));
    }
    Ok(dir.join(format!("{id}.json")))
}

fn summary(v: &Value) -> Option<Summary> {
    Some(Summary {
        id: v["id"].as_str()?.to_string(),
        title: v["title"].as_str().unwrap_or("Sans titre").to_string(),
        updated_at: v["updated_at"].as_i64().unwrap_or(0),
    })
}

pub fn list(app: &AppHandle) -> Result<Vec<Summary>, String> {
    let mut out: Vec<Summary> = std::fs::read_dir(dir(app)?)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
        .filter_map(|v| summary(&v))
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
    Ok(out)
}

pub fn load(app: &AppHandle, id: &str) -> Result<Value, String> {
    let raw = std::fs::read_to_string(file(&dir(app)?, id)?).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save(app: &AppHandle, conversation: &Value) -> Result<(), String> {
    let id = conversation["id"]
        .as_str()
        .ok_or("Discussion sans identifiant")?;
    let path = file(&dir(app)?, id)?;
    // Écriture atomique : un crash pendant l'écriture ne corrompt pas la discussion.
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(conversation).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    let path = file(&dir(app)?, id)?;
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        let dir = Path::new("/tmp/x");
        assert!(file(dir, "../../etc/passwd").is_err());
        assert!(file(dir, "").is_err());
        assert_eq!(
            file(dir, "3f2a-bc01").unwrap(),
            Path::new("/tmp/x/3f2a-bc01.json")
        );
    }

    #[test]
    fn summary_reads_list_fields() {
        let v: Value =
            serde_json::json!({"id": "a1", "title": "Q ?", "updated_at": 42, "turns": []});
        let s = summary(&v).unwrap();
        assert_eq!(
            (s.id.as_str(), s.title.as_str(), s.updated_at),
            ("a1", "Q ?", 42)
        );
    }
}
