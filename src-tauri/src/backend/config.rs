use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::dto::AppConfigDto;

const CONFIG_NAME: &str = "explorer_config.json";
const CONFIG_DIR: &str = "adams_file_explorer";
const LEGACY_CONFIG_DIR: &str = "file_explorer";

fn config_path_for(dir_name: &str) -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(dir_name).join(CONFIG_NAME)
}

pub fn config_path() -> PathBuf {
    config_path_for(CONFIG_DIR)
}

fn legacy_config_path() -> PathBuf {
    config_path_for(LEGACY_CONFIG_DIR)
}

fn migrate_legacy_config_if_needed(current: &Path, legacy: &Path) -> Result<(), String> {
    if current.exists() || !legacy.exists() {
        return Ok(());
    }

    if let Some(parent) = current.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("Failed to create config dir: {err}"))?;
    }

    fs::copy(legacy, current)
        .map(|_| ())
        .map_err(|err| format!("Failed to migrate config: {err}"))
}

pub fn load_config() -> Result<AppConfigDto, String> {
    let path = config_path();
    migrate_legacy_config_if_needed(&path, &legacy_config_path())?;

    if !path.exists() {
        return Ok(AppConfigDto::default());
    }

    let raw = fs::read_to_string(&path).map_err(|err| format!("Failed to read config: {err}"))?;
    serde_json::from_str::<AppConfigDto>(&raw)
        .map_err(|err| format!("Failed to parse config {}: {err}", path.display()))
}

pub fn save_config(config: &AppConfigDto) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("Failed to create config dir: {err}"))?;
    }

    let raw = serde_json::to_string_pretty(config)
        .map_err(|err| format!("Failed to serialize config: {err}"))?;

    fs::write(path, raw).map_err(|err| format!("Failed to write config: {err}"))
}

fn expand_home_shorthand(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(path));
    }

    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(path)
}

pub fn normalize_directory(path: &str) -> Result<String, String> {
    let path = expand_home_shorthand(path);
    let canonical = fs::canonicalize(path).map_err(|err| format!("Invalid directory: {err}"))?;

    if !canonical.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    Ok(canonical.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adams_file_explorer_config_{name}_{}",
            std::process::id()
        ))
    }

    fn canonical_string(path: PathBuf) -> String {
        fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn normalize_directory_expands_home_shorthand() {
        let home = dirs::home_dir().unwrap();

        assert_eq!(normalize_directory("~").unwrap(), canonical_string(home));
    }

    #[test]
    fn normalize_directory_expands_home_shorthand_with_separator() {
        let home = dirs::home_dir().unwrap();

        assert_eq!(normalize_directory("~/").unwrap(), canonical_string(home));
    }

    #[test]
    fn normalize_directory_keeps_absolute_paths() {
        let dir = test_dir("absolute_path");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        assert_eq!(
            normalize_directory(dir.to_str().unwrap()).unwrap(),
            canonical_string(dir.clone())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_directory_rejects_missing_home_child() {
        let path = format!(
            "~/adams_file_explorer_missing_home_child_{}",
            std::process::id()
        );

        assert!(normalize_directory(&path).is_err());
    }

    #[test]
    fn normalize_directory_does_not_expand_other_user_shorthand() {
        assert_eq!(
            expand_home_shorthand("~otheruser"),
            PathBuf::from("~otheruser")
        );
        assert_eq!(
            expand_home_shorthand("~otheruser/Documents"),
            PathBuf::from("~otheruser/Documents")
        );
    }

    #[test]
    fn migrate_legacy_config_copies_when_current_is_missing() {
        let base = test_dir("copy_missing");
        let _ = fs::remove_dir_all(&base);

        let legacy = base.join("legacy").join(CONFIG_NAME);
        let current = base.join("current").join(CONFIG_NAME);
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(
            &legacy,
            r#"{"favorites":[],"last_directory":"/tmp","open_with_map":{}}"#,
        )
        .unwrap();

        migrate_legacy_config_if_needed(&current, &legacy).unwrap();

        assert_eq!(
            fs::read_to_string(&current).unwrap(),
            fs::read_to_string(&legacy).unwrap()
        );
        assert!(legacy.exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_legacy_config_keeps_existing_current_config() {
        let base = test_dir("keep_current");
        let _ = fs::remove_dir_all(&base);

        let legacy = base.join("legacy").join(CONFIG_NAME);
        let current = base.join("current").join(CONFIG_NAME);
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        fs::write(&legacy, "legacy").unwrap();
        fs::write(&current, "current").unwrap();

        migrate_legacy_config_if_needed(&current, &legacy).unwrap();

        assert_eq!(fs::read_to_string(&current).unwrap(), "current");
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "legacy");

        let _ = fs::remove_dir_all(&base);
    }
}
