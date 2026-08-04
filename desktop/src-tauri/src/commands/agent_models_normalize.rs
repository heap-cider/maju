use std::collections::HashSet;

use crate::managed_agents::{AgentModelInfo, AgentModelsResponse};

pub(in crate::commands) fn normalize_agent_models(
    raw: &serde_json::Value,
    persisted_model: Option<String>,
) -> AgentModelsResponse {
    let config_options = super::super::agent_config::parse_config_options(
        raw.get("stable")
            .and_then(|stable| stable.get("configOptions")),
    );
    let agent_name = raw["agent"]["name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let agent_version = raw["agent"]["version"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let mut models = Vec::new();
    let mut seen_ids = HashSet::new();

    if let Some(config_options) = raw["stable"]["configOptions"].as_array() {
        for option in config_options {
            if option.get("category").and_then(|value| value.as_str()) != Some("model") {
                continue;
            }
            for entry in option
                .get("options")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
            {
                let Some(id) = entry.get("value").and_then(|value| value.as_str()) else {
                    continue;
                };
                if seen_ids.insert(id.to_string()) {
                    models.push(AgentModelInfo {
                        id: id.to_string(),
                        name: entry
                            .get("displayName")
                            .or_else(|| entry.get("name"))
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                        description: None,
                    });
                }
            }
        }
    }

    let mut agent_default_model = None;
    if let Some(unstable) = raw.get("unstable") {
        agent_default_model = unstable["currentModelId"].as_str().map(str::to_string);
        for entry in unstable["availableModels"].as_array().into_iter().flatten() {
            let Some(id) = entry.get("modelId").and_then(|value| value.as_str()) else {
                continue;
            };
            if seen_ids.insert(id.to_string()) {
                models.push(AgentModelInfo {
                    id: id.to_string(),
                    name: entry
                        .get("name")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    description: entry
                        .get("description")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                });
            }
        }
    }

    AgentModelsResponse {
        agent_name,
        agent_version,
        supports_switching: !models.is_empty(),
        models,
        agent_default_model,
        selected_model: persisted_model,
        config_options,
    }
}
