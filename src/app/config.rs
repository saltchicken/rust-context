use crate::app::cli::Cli;
use crate::app::models::RuntimeConfig;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
struct PresetsFile {
    #[serde(flatten)]
    presets: HashMap<String, PresetConfig>,
}

#[derive(Deserialize, Debug, Clone, Default)]
struct PresetConfig {
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    include_in_tree: Option<Vec<String>>,
}

/// ‼️ REFACTOR: Extracted preset loading to its own helper function
fn load_presets_file() -> Result<HashMap<String, PresetConfig>> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let config_path = home
        .join(".config")
        .join("code_context")
        .join("presets.toml");

    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(&config_path)
        .context(format!("Failed to read config at {:?}", config_path))?;

    let parsed: PresetsFile = toml::from_str(&content).context("Failed to parse presets.toml")?;

    Ok(parsed.presets)
}

/// ‼️ REFACTOR: Extracted merging logic to keep the resolve function clean
fn merge_vecs(preset_vec: Option<Vec<String>>, cli_vec: Option<Vec<String>>) -> Vec<String> {
    let mut combined = preset_vec.unwrap_or_default();
    if let Some(mut cli_items) = cli_vec {
        combined.append(&mut cli_items);
    }
    // Deduplicate while keeping order
    let mut seen = std::collections::HashSet::new();
    combined.retain(|item| seen.insert(item.clone()));
    combined
}

pub fn resolve_config(cli: Cli, project_name: Option<&str>) -> Result<RuntimeConfig> {
    let presets = load_presets_file()?;

    // Determine preset to use: CLI flag > Auto-detect > None
    let preset_key = cli.preset.as_deref().or(project_name);
    let preset = preset_key
        .and_then(|k| presets.get(k))
        .cloned()
        .unwrap_or_default();

    let config = RuntimeConfig {
        include: merge_vecs(preset.include, cli.include),
        exclude: merge_vecs(preset.exclude, cli.exclude),
        include_in_tree: merge_vecs(preset.include_in_tree, cli.include_in_tree),
        tree_only_output: cli.tree,
    };

    Ok(config)
}
