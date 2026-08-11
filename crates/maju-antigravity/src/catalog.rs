use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawModel {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelVariant {
    pub effort: String,
    pub raw_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFamily {
    pub id: String,
    pub label: String,
    pub variants: Vec<ModelVariant>,
    pub raw_id: String,
}

impl ModelFamily {
    pub fn efforts(&self) -> impl Iterator<Item = &str> {
        self.variants.iter().map(|variant| variant.effort.as_str())
    }

    pub fn command_selection(&self, effort: Option<&str>) -> (&str, Option<&str>) {
        if self.variants.is_empty() {
            return (&self.raw_id, None);
        }
        let selected = effort
            .and_then(|value| self.variants.iter().find(|variant| variant.effort == value))
            .unwrap_or(&self.variants[0]);
        (&self.id, Some(selected.effort.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    pub families: Vec<ModelFamily>,
}

impl ModelCatalog {
    pub fn from_raw(models: Vec<RawModel>) -> Result<Self, String> {
        if models.is_empty() {
            return Err("Antigravity returned no models".to_string());
        }

        #[derive(Default)]
        struct CandidateGroup {
            first_index: usize,
            label: String,
            variants: Vec<ModelVariant>,
        }

        let mut groups: HashMap<String, CandidateGroup> = HashMap::new();
        let mut singles = Vec::new();
        for (index, model) in models.iter().enumerate() {
            if let Some((base_id, base_label, effort)) = split_effort_variant(model) {
                let group = groups.entry(base_id).or_insert_with(|| CandidateGroup {
                    first_index: index,
                    label: base_label,
                    variants: Vec::new(),
                });
                group.variants.push(ModelVariant {
                    effort,
                    raw_id: model.id.clone(),
                });
            } else {
                singles.push((index, model.clone()));
            }
        }

        let mut ordered = Vec::new();
        for (base_id, group) in groups {
            if group.variants.len() >= 2 {
                ordered.push((
                    group.first_index,
                    ModelFamily {
                        id: base_id.clone(),
                        label: group.label,
                        raw_id: base_id,
                        variants: group.variants,
                    },
                ));
            } else if let Some(variant) = group.variants.into_iter().next() {
                let raw = models
                    .get(group.first_index)
                    .cloned()
                    .ok_or_else(|| "invalid Antigravity model catalog index".to_string())?;
                ordered.push((
                    group.first_index,
                    ModelFamily {
                        id: raw.id.clone(),
                        label: raw.label,
                        raw_id: variant.raw_id,
                        variants: Vec::new(),
                    },
                ));
            }
        }
        for (index, raw) in singles {
            ordered.push((
                index,
                ModelFamily {
                    id: raw.id.clone(),
                    label: raw.label,
                    raw_id: raw.id,
                    variants: Vec::new(),
                },
            ));
        }
        ordered.sort_by_key(|(index, _)| *index);

        Ok(Self {
            families: ordered.into_iter().map(|(_, family)| family).collect(),
        })
    }

    pub fn family(&self, id: &str) -> Option<&ModelFamily> {
        self.families.iter().find(|family| family.id == id)
    }
}

fn split_effort_variant(model: &RawModel) -> Option<(String, String, String)> {
    let (base_label, suffix) = model.label.rsplit_once(" (")?;
    let effort = suffix.strip_suffix(')')?.trim().to_ascii_lowercase();
    if effort.is_empty()
        || !effort
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '-')
    {
        return None;
    }
    let base_id = model.id.strip_suffix(&format!("-{effort}"))?;
    if base_id.is_empty() || base_label.is_empty() {
        return None;
    }
    Some((base_id.to_string(), base_label.to_string(), effort))
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    discovered_at: u64,
    models: Vec<RawModel>,
}

pub async fn discover(agy_command: &str) -> Result<ModelCatalog, String> {
    let cache_path = model_cache_path();
    if let Some(models) = read_cache(&cache_path, true).await {
        return ModelCatalog::from_raw(models);
    }

    match discover_live(agy_command).await {
        Ok(models) => {
            if let Err(error) = write_cache(&cache_path, &models).await {
                tracing::warn!("could not cache Antigravity models: {error}");
            }
            ModelCatalog::from_raw(models)
        }
        Err(live_error) => {
            if let Some(models) = read_cache(&cache_path, false).await {
                tracing::warn!(
                    "live Antigravity model discovery failed; using stale cache: {live_error}"
                );
                return ModelCatalog::from_raw(models);
            }
            Err(live_error)
        }
    }
}

fn model_cache_path() -> PathBuf {
    if let Some(path) = std::env::var_os("MAJU_ANTIGRAVITY_CACHE_DIR") {
        return PathBuf::from(path).join("models.json");
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("maju")
        .join("antigravity")
        .join("models.json")
}

async fn read_cache(path: &Path, require_fresh: bool) -> Option<Vec<RawModel>> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let cache: CacheFile = serde_json::from_slice(&bytes).ok()?;
    if cache.models.is_empty() {
        return None;
    }
    if require_fresh {
        let now = unix_seconds();
        if now.saturating_sub(cache.discovered_at) > CACHE_TTL.as_secs() {
            return None;
        }
    }
    Some(cache.models)
}

async fn write_cache(path: &Path, models: &[RawModel]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "model cache path has no parent".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&CacheFile {
        discovered_at: unix_seconds(),
        models: models.to_vec(),
    })
    .map_err(|error| error.to_string())?;
    tokio::fs::write(path, bytes)
        .await
        .map_err(|error| error.to_string())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn discover_live(agy_command: &str) -> Result<Vec<RawModel>, String> {
    use std::io::{Read, Seek, SeekFrom};

    // `agy` may briefly leave an updater descendant alive. Pipe-based output
    // waits for that descendant to close inherited handles even after the real
    // CLI exits. Regular files keep discovery bounded on every OS.
    let mut stdout_file = tempfile::tempfile()
        .map_err(|error| format!("could not create model discovery output: {error}"))?;
    let mut stderr_file = tempfile::tempfile()
        .map_err(|error| format!("could not create model discovery error output: {error}"))?;
    let mut command = Command::new(agy_command);
    command.arg("models");
    command.kill_on_drop(true);
    command.stdin(std::process::Stdio::null());
    command.stdout(
        stdout_file
            .try_clone()
            .map_err(|error| format!("could not capture model output: {error}"))?,
    );
    command.stderr(
        stderr_file
            .try_clone()
            .map_err(|error| format!("could not capture model errors: {error}"))?,
    );
    configure_no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("could not start `{agy_command} models`: {error}"))?;
    let status = tokio::time::timeout(DISCOVERY_TIMEOUT, async {
        loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("could not wait for `{agy_command} models`: {error}"))?
            {
                return Ok::<_, String>(status);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .map_err(|_| {
        let _ = child.start_kill();
        "Antigravity model discovery timed out".to_string()
    })??;

    stdout_file
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("could not seek model output: {error}"))?;
    stderr_file
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("could not seek model errors: {error}"))?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    stdout_file
        .take(1024 * 1024)
        .read_to_end(&mut stdout)
        .map_err(|error| format!("could not read model output: {error}"))?;
    stderr_file
        .take(64 * 1024)
        .read_to_end(&mut stderr)
        .map_err(|error| format!("could not read model errors: {error}"))?;

    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        return Err(format!("`{agy_command} models` failed: {}", stderr.trim()));
    }
    parse_models_output(&String::from_utf8_lossy(&stdout))
}

pub fn parse_models_output(stdout: &str) -> Result<Vec<RawModel>, String> {
    let models = stdout
        .lines()
        .filter_map(|line| {
            let (id, label) = line.split_once('\t')?;
            let id = id.trim();
            let label = label.trim();
            (!id.is_empty() && !label.is_empty()).then(|| RawModel {
                id: id.to_string(),
                label: label.to_string(),
            })
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        return Err("Antigravity returned no parseable models".to_string());
    }
    Ok(models)
}

fn configure_no_window(_command: &mut Command) {
    #[cfg(windows)]
    _command.creation_flags(0x0800_0000);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: &str, label: &str) -> RawModel {
        RawModel {
            id: id.to_string(),
            label: label.to_string(),
        }
    }

    #[test]
    fn groups_only_live_multi_effort_variants() {
        let catalog = ModelCatalog::from_raw(vec![
            raw("gemini-3.6-flash-high", "Gemini 3.6 Flash (High)"),
            raw("gemini-3.6-flash-medium", "Gemini 3.6 Flash (Medium)"),
            raw("gemini-3.6-flash-low", "Gemini 3.6 Flash (Low)"),
            raw("gemini-3.6-flash-max", "Gemini 3.6 Flash (Max)"),
            raw("gemini-3.6-flash-off", "Gemini 3.6 Flash (Off)"),
            raw("claude-opus-4-6-thinking", "Claude Opus 4.6 (Thinking)"),
            raw("gpt-oss-120b-medium", "GPT-OSS 120B (Medium)"),
        ])
        .unwrap();

        assert_eq!(catalog.families[0].id, "gemini-3.6-flash");
        assert_eq!(catalog.families[0].label, "Gemini 3.6 Flash");
        assert_eq!(
            catalog.families[0].efforts().collect::<Vec<_>>(),
            vec!["high", "medium", "low", "max", "off"]
        );
        assert_eq!(catalog.families[1].id, "claude-opus-4-6-thinking");
        assert!(catalog.families[1].variants.is_empty());
        assert_eq!(catalog.families[2].id, "gpt-oss-120b-medium");
        assert!(catalog.families[2].variants.is_empty());
    }

    #[test]
    fn command_selection_uses_base_model_and_selected_live_effort() {
        let catalog = ModelCatalog::from_raw(vec![
            raw("gemini-3.6-flash-high", "Gemini 3.6 Flash (High)"),
            raw("gemini-3.6-flash-low", "Gemini 3.6 Flash (Low)"),
        ])
        .unwrap();
        assert_eq!(
            catalog.families[0].command_selection(Some("low")),
            ("gemini-3.6-flash", Some("low"))
        );
    }

    #[test]
    fn parser_ignores_diagnostics_without_tabs() {
        let models = parse_models_output(
            "gemini-3.6-flash-high\tGemini 3.6 Flash (High)\nFetching models...\n",
        )
        .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gemini-3.6-flash-high");
    }
}
