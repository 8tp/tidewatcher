use std::{
    env, fs,
    path::{Path, PathBuf},
};

const DEFAULT_REFRESH_INTERVAL_MS: u64 = 2_000;
const DEFAULT_ANIMATION_INTERVAL_MS: u64 = 180;
const DEFAULT_HISTORY_CAPACITY: usize = 60;
const DEFAULT_THEME: &str = "Sakura Tide";
const DEFAULT_PROCESS_SORT: &str = "cpu";
const DEFAULT_DISK_LIMIT: usize = 4;
const DEFAULT_NETWORK_LIMIT: usize = 4;

const MIN_REFRESH_INTERVAL_MS: u64 = 250;
const MAX_REFRESH_INTERVAL_MS: u64 = 10_000;
const MIN_ANIMATION_INTERVAL_MS: u64 = 80;
const MAX_ANIMATION_INTERVAL_MS: u64 = 1_000;
const MIN_HISTORY_CAPACITY: usize = 20;
const MAX_HISTORY_CAPACITY: usize = 240;
const MIN_ENTITY_LIMIT: usize = 1;
const MAX_ENTITY_LIMIT: usize = 8;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub refresh_interval_ms: u64,
    pub animation_interval_ms: u64,
    pub history_capacity: usize,
    pub theme: String,
    pub process_sort: String,
    pub disk_limit: usize,
    pub network_limit: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            animation_interval_ms: DEFAULT_ANIMATION_INTERVAL_MS,
            history_capacity: DEFAULT_HISTORY_CAPACITY,
            theme: DEFAULT_THEME.to_string(),
            process_sort: DEFAULT_PROCESS_SORT.to_string(),
            disk_limit: DEFAULT_DISK_LIMIT,
            network_limit: DEFAULT_NETWORK_LIMIT,
        }
    }
}

#[derive(Debug)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub warnings: Vec<String>,
}

pub fn load() -> LoadedConfig {
    let mut config = AppConfig::default();
    let mut warnings = Vec::new();
    let Some(path) = discover_config_path() else {
        return LoadedConfig { config, warnings };
    };

    match fs::read_to_string(&path) {
        Ok(contents) => parse_config(&contents, &mut config, &mut warnings),
        Err(error) => warnings.push(format!(
            "failed to read config at {}: {error}",
            path.display()
        )),
    }

    config.refresh_interval_ms = config
        .refresh_interval_ms
        .clamp(MIN_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS);
    config.animation_interval_ms = config
        .animation_interval_ms
        .clamp(MIN_ANIMATION_INTERVAL_MS, MAX_ANIMATION_INTERVAL_MS);
    config.history_capacity = config
        .history_capacity
        .clamp(MIN_HISTORY_CAPACITY, MAX_HISTORY_CAPACITY);
    config.disk_limit = config.disk_limit.clamp(MIN_ENTITY_LIMIT, MAX_ENTITY_LIMIT);
    config.network_limit = config
        .network_limit
        .clamp(MIN_ENTITY_LIMIT, MAX_ENTITY_LIMIT);

    LoadedConfig { config, warnings }
}

fn discover_config_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("TIDEWATCHER_CONFIG") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    let cwd_path = PathBuf::from("tidewatcher.toml");
    if cwd_path.exists() {
        return Some(cwd_path);
    }

    let home = env::var("HOME").ok()?;
    let config_path = Path::new(&home)
        .join(".config")
        .join("tidewatcher")
        .join("config.toml");
    config_path.exists().then_some(config_path)
}

fn parse_config(contents: &str, config: &mut AppConfig, warnings: &mut Vec<String>) {
    for (index, raw_line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let line = strip_comments(raw_line).trim();

        if line.is_empty() || line.starts_with('[') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            warnings.push(format!("line {line_number}: expected `key = value`"));
            continue;
        };

        let key = key.trim();
        let value = parse_string(value.trim());

        match key {
            "refresh_interval_ms" => match value.parse::<u64>() {
                Ok(parsed) => config.refresh_interval_ms = parsed,
                Err(_) => warnings.push(format!(
                    "line {line_number}: invalid refresh_interval_ms `{value}`"
                )),
            },
            "animation_interval_ms" => match value.parse::<u64>() {
                Ok(parsed) => config.animation_interval_ms = parsed,
                Err(_) => warnings.push(format!(
                    "line {line_number}: invalid animation_interval_ms `{value}`"
                )),
            },
            "history_capacity" => match value.parse::<usize>() {
                Ok(parsed) => config.history_capacity = parsed,
                Err(_) => warnings.push(format!(
                    "line {line_number}: invalid history_capacity `{value}`"
                )),
            },
            "theme" => config.theme = value,
            "process_sort" => config.process_sort = value,
            "disk_limit" => match value.parse::<usize>() {
                Ok(parsed) => config.disk_limit = parsed,
                Err(_) => {
                    warnings.push(format!("line {line_number}: invalid disk_limit `{value}`"))
                }
            },
            "network_limit" => match value.parse::<usize>() {
                Ok(parsed) => config.network_limit = parsed,
                Err(_) => warnings.push(format!(
                    "line {line_number}: invalid network_limit `{value}`"
                )),
            },
            _ => warnings.push(format!("line {line_number}: unknown key `{key}`")),
        }
    }
}

fn strip_comments(line: &str) -> &str {
    let mut in_quotes = false;

    for (index, character) in line.char_indices() {
        match character {
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return &line[..index],
            _ => {}
        }
    }

    line
}

fn parse_string(value: &str) -> String {
    let trimmed = value.trim();

    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_config, AppConfig};

    #[test]
    fn defaults_to_sakura_tide_theme() {
        assert_eq!(AppConfig::default().theme, "Sakura Tide");
    }

    #[test]
    fn parser_updates_known_keys() {
        let mut config = AppConfig::default();
        let mut warnings = Vec::new();

        parse_config(
            r#"
            refresh_interval_ms = 1500
            animation_interval_ms = 120
            history_capacity = 90
            theme = "Harbor Fog"
            process_sort = "memory"
            disk_limit = 6
            network_limit = 5
            "#,
            &mut config,
            &mut warnings,
        );

        assert_eq!(config.refresh_interval_ms, 1_500);
        assert_eq!(config.animation_interval_ms, 120);
        assert_eq!(config.history_capacity, 90);
        assert_eq!(config.theme, "Harbor Fog");
        assert_eq!(config.process_sort, "memory");
        assert_eq!(config.disk_limit, 6);
        assert_eq!(config.network_limit, 5);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parser_reports_unknown_keys() {
        let mut config = AppConfig::default();
        let mut warnings = Vec::new();

        parse_config("mystery = 1", &mut config, &mut warnings);

        assert_eq!(warnings.len(), 1);
    }
}
