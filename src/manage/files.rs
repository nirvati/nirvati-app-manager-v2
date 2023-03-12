use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use cached::proc_macro::once;
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::composegenerator::types::{AppYml, MetadataYml, OutputMetadata};

use super::ports::PortMapEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SimpleValue {
    String(String),
    Number(u64),
    Float(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserJson {
    name: String,
    password: String,
    #[serde(rename = "installedApps")]
    installed_apps: Vec<String>,
    https: Option<serde_json::Value>,
    #[serde(rename = "appSettings", default)]
    app_settings: HashMap<String, HashMap<String, SimpleValue>>,
    #[serde(rename = "nextAppRegen", default)]
    // The time app config files need to be regenerated, in seconds since epoch
    next_app_regen: u64,
}

/// Read the app registry
pub fn get_app_registry(nirvati_dir: &Path) -> Result<Vec<OutputMetadata>> {
    let app_registry_path = nirvati_dir.join("apps").join("registry.json");
    let app_registry = std::fs::File::open(app_registry_path)?;
    let app_registry: Vec<OutputMetadata> = serde_json::from_reader(app_registry)?;
    Ok(app_registry)
}

pub fn write_app_registry(nirvati_dir: &Path, app_registry: &[OutputMetadata]) -> Result<()> {
    let app_registry_path = nirvati_dir.join("apps").join("registry.json");
    let app_registry = serde_json::to_string_pretty(app_registry)?;
    std::fs::write(app_registry_path, app_registry)?;
    Ok(())
}

/// Reads the user's user.json config file
pub fn get_user_json(nirvati_dir: &Path) -> Result<UserJson> {
    let user_json_path = nirvati_dir.join("db").join("user.json");
    let user_json = std::fs::read_to_string(user_json_path)?;
    let user_json: UserJson = serde_json::from_str(&user_json)?;
    Ok(user_json)
}

/// Reads the user's user.json config file
/// Falls back to default values if it doesn't exist
pub fn get_user_json_default(nirvati_dir: &Path) -> Result<UserJson> {
    let user_json_path = nirvati_dir.join("db").join("user.json");
    if !user_json_path.exists() {
        let user_json = UserJson {
            name: "Unknown".to_string(),
            password: "Unknown".to_string(),
            installed_apps: Vec::new(),
            https: None,
            app_settings: HashMap::new(),
            next_app_regen: 0,
        };
        return Ok(user_json);
    }
    let user_json = std::fs::read_to_string(user_json_path)?;
    let user_json: UserJson = serde_json::from_str(&user_json)?;
    Ok(user_json)
}

pub fn get_installed_apps(nirvati_dir: &Path) -> Result<Vec<String>> {
    let user_json = get_user_json_default(nirvati_dir)?;
    Ok(user_json.installed_apps)
}

pub fn get_app_settings(
    nirvati_dir: &Path,
    app_id: &str,
) -> Result<Option<HashMap<String, SimpleValue>>> {
    let user_json = get_user_json_default(nirvati_dir)?;
    Ok(user_json.app_settings.get(app_id).cloned())
}

pub fn add_installed_app(app_id: &str, nirvati_dir: &Path) -> Result<()> {
    // Serialize the user.json as serde_json::Value to avoid accidentally deleting fields
    let user_json_path = nirvati_dir.join("db").join("user.json");
    let user_json = std::fs::read_to_string(&user_json_path)?;
    let mut user_json: serde_json::Value = serde_json::from_str(&user_json)?;
    let app_list = user_json
        .as_object_mut()
        .ok_or_else(|| anyhow!("user.json is not an object"))?
        .get_mut("installedApps")
        .ok_or_else(|| anyhow!("user.json does not contain installedApps"))?
        .as_array_mut()
        .ok_or_else(|| anyhow!("installedApps is not an array"))?;
    if !app_list.contains(&serde_json::Value::String(app_id.to_string())) {
        app_list.push(serde_json::Value::String(app_id.to_string()));
    }
    let user_json = serde_json::to_string_pretty(&user_json)?;
    std::fs::write(user_json_path, user_json)?;
    Ok(())
}

pub fn remove_installed_app(app_id: &str, nirvati_dir: &Path) -> Result<()> {
    // Serialize the user.json as serde_json::Value to avoid accidentally deleting fields
    let user_json_path = nirvati_dir.join("db").join("user.json");
    let user_json = std::fs::read_to_string(&user_json_path)?;
    let mut user_json: serde_json::Value = serde_json::from_str(&user_json)?;
    let installed_apps = user_json
        .as_object_mut()
        .ok_or_else(|| anyhow!("user.json is not an object"))?
        .get_mut("installedApps")
        .ok_or_else(|| anyhow!("user.json does not contain installedApps"))?
        .as_array_mut()
        .ok_or_else(|| anyhow!("installedApps is not an array"))?;
    let mut index = None;
    for (i, app) in installed_apps.iter().enumerate() {
        if app
            .as_str()
            .ok_or_else(|| anyhow!("installedApps is not an array of strings"))?
            == app_id
        {
            index = Some(i);
            break;
        }
    }
    if let Some(index) = index {
        installed_apps.remove(index);
    }
    let user_json = serde_json::to_string_pretty(&user_json)?;
    std::fs::write(user_json_path, user_json)?;
    Ok(())
}

pub fn get_next_app_regenerate(nirvati_dir: &Path) -> Result<u64> {
    let user_json = get_user_json_default(nirvati_dir)?;
    Ok(user_json.next_app_regen)
}

pub fn set_next_app_regenerate(nirvati_dir: &Path, time: u64) -> Result<()> {
    // Serialize the user.json as serde_json::Value to avoid accidentally deleting fields
    let user_json_path = nirvati_dir.join("db").join("user.json");
    let user_json = std::fs::read_to_string(&user_json_path)?;
    let mut user_json: serde_json::Value = serde_json::from_str(&user_json)?;
    let next_app_regen = user_json
        .as_object_mut()
        .ok_or_else(|| anyhow!("user.json is not an object"))?
        .get_mut("nextAppRegen")
        .ok_or_else(|| anyhow!("user.json does not contain nextAppRegen"))?;
    *next_app_regen = serde_json::Value::Number(serde_json::Number::from(time));
    let user_json = serde_json::to_string_pretty(&user_json)?;
    std::fs::write(user_json_path, user_json)?;
    Ok(())
}

#[once(sync_writes = true, time = 10000)]
pub fn app_requires_settings(nirvati_dir: &Path, app_name: &str) -> bool {
    let settings_yml_path = nirvati_dir.join("apps").join(app_name).join("settings.yml");
    settings_yml_path.exists()
}

pub fn save_app_settings(
    app_id: &str,
    settings: HashMap<String, SimpleValue>,
    nirvati_dir: &Path,
) -> Result<()> {
    // Serialize the user.json as serde_json::Value to avoid accidentally deleting fields
    let user_json_path = nirvati_dir.join("db").join("user.json");
    let user_json = std::fs::read_to_string(&user_json_path)?;
    let mut user_json: serde_json::Value = serde_json::from_str(&user_json)?;
    let user_json_obj = user_json
        .as_object_mut()
        .ok_or_else(|| anyhow!("user.json is not an object"))?;
    if !user_json_obj.contains_key("appSettings") {
        user_json_obj.insert(
            "appSettings".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }
    user_json_obj
        .get_mut("appSettings")
        .ok_or_else(|| anyhow!("user.json does not contain appSettings"))?
        .as_object_mut()
        .ok_or_else(|| anyhow!("appSettings is not an object"))?
        .insert(
            app_id.to_string(),
            serde_json::Value::Object(
                settings
                    .into_iter()
                    .map(|(k, v)| -> Result<(String, serde_json::Value)> {
                        Ok((
                            k,
                            match v {
                                SimpleValue::String(s) => serde_json::Value::String(s),
                                SimpleValue::Number(n) => {
                                    serde_json::Value::Number(serde_json::Number::from(n))
                                }
                                SimpleValue::Float(f) => serde_json::Value::Number(
                                    serde_json::Number::from_f64(f)
                                        .ok_or_else(|| anyhow!("float is not a number"))?,
                                ),
                            },
                        ))
                    })
                    .collect::<Result<Map<String, serde_json::Value>>>()?,
            ),
        );
    let user_json = serde_json::to_string_pretty(&user_json)?;
    std::fs::write(user_json_path, user_json)?;
    Ok(())
}

pub fn get_available_permissions(nirvati_dir: &Path) -> Result<Vec<String>> {
    let permissions_json_path = nirvati_dir.join("apps").join("permissions.json");
    if permissions_json_path.exists() {
        let permissions_json = std::fs::read_to_string(permissions_json_path)?;
        let permissions_json: Vec<String> = serde_json::from_str(&permissions_json)?;
        Ok(permissions_json)
    } else {
        Ok(Vec::new())
    }
}

pub fn save_permissions(nirvati_dir: &Path, permissions: Vec<String>) -> Result<()> {
    let permissions_json_path = nirvati_dir.join("apps").join("permissions.json");
    let permissions_json = serde_json::to_string(&permissions)?;
    std::fs::write(permissions_json_path, permissions_json)?;
    Ok(())
}

pub fn get_port_map(nirvati_dir: &Path) -> Result<Vec<PortMapEntry>> {
    let port_map_yml_path = nirvati_dir.join("apps").join("ports.yml");
    if port_map_yml_path.exists() {
        let port_map_yml = std::fs::read_to_string(port_map_yml_path)?;
        let port_map_yml: Vec<PortMapEntry> = serde_yaml::from_str(&port_map_yml)?;
        Ok(port_map_yml)
    } else {
        Ok(Vec::new())
    }
}

pub fn save_port_map(nirvati_dir: &Path, port_map: Vec<PortMapEntry>) -> Result<()> {
    let port_map_yml_path = nirvati_dir.join("apps").join("ports.yml");
    let port_map_yml = serde_yaml::to_string(&port_map)?;
    std::fs::write(port_map_yml_path, port_map_yml)?;
    Ok(())
}

//#[once(sync_writes = true, time = 10000, result = true)]
pub fn read_app_yml(nirvati_dir: &Path, app_name: &str) -> Result<AppYml> {
    let app_yml_path = nirvati_dir.join("apps").join(app_name).join("app.yml");
    let app_yml: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(app_yml_path)?)?;
    let app_version = app_yml
        .get("version")
        .ok_or_else(|| anyhow!("app.yml does not contain a version"))?
        .as_i64()
        .ok_or_else(|| anyhow!("app.yml version is not an integer"))?;
    match app_version {
        1 => {
            let app_yml = AppYml::V1(serde_yaml::from_value(app_yml)?);
            Ok(app_yml)
        }
        _ => Err(anyhow!("app.yml version is not supported")),
    }
}

//#[once(sync_writes = true, time = 10000, result = true)]
pub fn read_metadata_yml(nirvati_dir: &Path, app_name: &str) -> Result<MetadataYml> {
    let metadata_yml_path = nirvati_dir.join("apps").join(app_name).join("metadata.yml");
    let metadata_yml: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(metadata_yml_path)?)?;
    let metadata_version = metadata_yml
        .get("version")
        .ok_or_else(|| anyhow!("metadata.yml does not contain a version"))?
        .as_i64()
        .ok_or_else(|| anyhow!("metadata.yml version is not an integer"))?;
    match metadata_version {
        1 => {
            let metadata_yml = MetadataYml::V1(serde_yaml::from_value(metadata_yml)?);
            Ok(metadata_yml)
        }
        _ => Err(anyhow!("metadata.yml version is not supported")),
    }
}

pub fn get_all_metadata_ymls(nirvati_dir: &Path) -> Result<Vec<OutputMetadata>> {
    let mut metadata_ymls = Vec::new();
    for entry in std::fs::read_dir(nirvati_dir.join("apps"))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let app_id = entry.file_name().to_str().unwrap().to_owned();
        if let Ok(metadata_yml) = read_metadata_yml(nirvati_dir, &app_id) {
            metadata_ymls.push(metadata_yml.into_basic_output_metadata(app_id));
        }
    }
    Ok(metadata_ymls)
}
