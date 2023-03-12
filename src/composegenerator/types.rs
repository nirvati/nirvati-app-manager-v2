use anyhow::{anyhow, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

use crate::{
    composegenerator::output::types::ComposeSpecification,
    manage::ports::PortMapEntry,
    utils::{find_env_vars, is_false},
};

// General types also relevant for the output
// Can be re-used by schemas

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, JsonSchema)]
#[serde(untagged)]
pub enum Command {
    SimpleCmd(String),
    ArraySyntax(Vec<String>),
}

impl Command {
    pub fn get_env_vars(&self) -> Vec<&str> {
        match self {
            Command::SimpleCmd(cmd) => find_env_vars(cmd),
            Command::ArraySyntax(cmd) => {
                let mut env_vars = Vec::new();
                for cmd_part in cmd {
                    env_vars.extend(find_env_vars(cmd_part));
                }
                env_vars
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, JsonSchema)]
#[serde(untagged)]
pub enum Dependency {
    OneDependency(String),
    AlternativeDependency(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default = "Vec::default")]
    #[serde(skip_serializing_if = "Vec::<String>::is_empty")]
    /// Other permissions this permission implies
    /// May also contain permissions of other apps
    pub includes: Vec<String>,
    #[serde(default = "BTreeMap::default")]
    #[serde(skip_serializing_if = "BTreeMap::<String, Value>::is_empty")]
    /// Variables accessible with this permission
    /// Strings here are accessible as env vars,
    /// any other type only in Jinja
    pub variables: BTreeMap<String, Value>,
    #[serde(default = "Vec::default")]
    #[serde(skip_serializing_if = "Vec::<String>::is_empty")]
    /// Files accessible with this permission
    pub files: Vec<String>,
    /// Makes this permission "invisible" (Hidden from the UI)
    #[serde(default = "bool::default")]
    #[serde(skip_serializing_if = "is_false")]
    pub hidden: bool,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputMetadata {
    /// The app id, only set in output
    pub id: String,
    /// The name of the app
    pub name: String,
    /// The version of the app
    pub version: String,
    /// The category for the app
    pub category: String,
    /// A short tagline for the app
    pub tagline: String,
    // Developer name -> their website
    pub developers: BTreeMap<String, String>,
    /// A description of the app
    pub description: String,
    #[serde(default)]
    /// Dependencies the app requires
    pub dependencies: Vec<Dependency>,
    /// Other permissions the app has
    pub has_permissions: Vec<String>,
    /// App repository name -> repo URL
    pub repo: BTreeMap<String, String>,
    /// A support link for the app
    pub support: String,
    /// A list of promo images for the apps
    pub gallery: Option<Vec<String>>,
    /// The path the "Open" link on the dashboard should lead to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The app's default username
    pub default_username: Option<String>,
    /// The app's default password. Can also be $APP_SEED for a random password
    pub default_password: Option<String>,
    #[serde(default = "bool::default")]
    /// True if the app only works over Tor
    pub tor_only: bool,
    /// A list of containers to update automatically (still validated by the Citadel team)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_containers: Option<Vec<String>>,
    /// For "virtual" apps, the service the app implements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_control: Option<String>,
    /// True if all dependencies are installed
    pub compatible: bool,
    pub port: u16,
    pub internal_port: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::<String, String>::is_empty")]
    pub release_notes: BTreeMap<String, String>,
    pub supports_https: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, JsonSchema)]
pub struct CaddyEntry {
    pub public_port: u16,
    pub internal_port: u16,
    pub container_name: String,
    pub is_primary: bool,
    pub is_l4: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, JsonSchema, Default)]
pub struct ResultYml {
    pub caddy_entries: Vec<CaddyEntry>,
    pub spec: ComposeSpecification,
    pub metadata: OutputMetadata,
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AppYml {
    V1(super::v1::types::AppYml),
}

impl AppYml {
    pub fn get_config_jinja_permissions(&self) -> &Vec<String> {
        match self {
            AppYml::V1(app) => &app.metadata.jinja_config_permissions,
        }
    }

    pub fn into_config_jinja_permissions(self) -> Vec<String> {
        match self {
            AppYml::V1(app) => app.metadata.jinja_config_permissions,
        }
    }

    pub fn get_exported_permissions(&self) -> &Vec<Permission> {
        match self {
            AppYml::V1(app) => &app.metadata.permissions,
        }
    }

    pub fn into_exported_permissions(self) -> Vec<Permission> {
        match self {
            AppYml::V1(app) => app.metadata.permissions,
        }
    }

    pub fn get_ports(&self, app_id: &str, implements: Option<String>) -> Vec<PortMapEntry> {
        match self {
            AppYml::V1(app) => app.get_ports(app_id, implements),
        }
    }

    pub fn convert(
        &self,
        app_id: &str,
        port_map: &[PortMapEntry],
        metadata: MetadataYml,
        available_permissions: &HashMap<String, Vec<Permission>>,
    ) -> Result<ResultYml> {
        match self {
            AppYml::V1(app) => {
                #[allow(irrefutable_let_patterns)]
                let MetadataYml::V1(metadata) = metadata else {
                    return Err(anyhow!("Invalid metadata"));
                };
                super::v1::convert::convert_app_yml(
                    app_id,
                    app,
                    metadata.metadata,
                    port_map,
                    available_permissions,
                )
            }
        }
    }
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum MetadataYml {
    V1(super::v1::types::MetadataYml),
}

impl MetadataYml {
    pub fn get_app_yml_jinja_permissions(&self) -> &Vec<String> {
        match self {
            MetadataYml::V1(metadata) => &metadata.metadata.app_yml_jinja_permissions,
        }
    }

    pub fn into_app_yml_jinja_permissions(self) -> Vec<String> {
        match self {
            MetadataYml::V1(metadata) => metadata.metadata.app_yml_jinja_permissions,
        }
    }

    pub fn into_basic_output_metadata(self, app_id: String) -> OutputMetadata {
        match self {
            MetadataYml::V1(metadata) => OutputMetadata {
                id: app_id,
                name: metadata.metadata.name,
                version: metadata.metadata.version,
                category: metadata.metadata.category,
                tagline: metadata.metadata.tagline,
                developers: metadata.metadata.developers,
                description: metadata.metadata.description,
                dependencies: metadata.metadata.dependencies,
                has_permissions: metadata.metadata.app_yml_jinja_permissions,
                repo: metadata.metadata.repo,
                support: metadata.metadata.support,
                gallery: metadata.metadata.gallery,
                path: metadata.metadata.path,
                default_username: metadata.metadata.default_username,
                default_password: metadata.metadata.default_password,
                tor_only: metadata.metadata.tor_only,
                update_containers: metadata.metadata.update_containers,
                implements: metadata.metadata.implements,
                version_control: metadata.metadata.version_control,
                // This is only metadata for an app that's not installable, so compatible can never be true
                compatible: false,
                release_notes: metadata.metadata.release_notes,
                port: 0,
                internal_port: 0,
                supports_https: false,
            },
        }
    }

    pub fn get_basic_output_metadata(&self, app_id: String) -> OutputMetadata {
        match self {
            MetadataYml::V1(metadata) => {
                let metadata = metadata.metadata.clone();
                OutputMetadata {
                    id: app_id,
                    name: metadata.name,
                    version: metadata.version,
                    category: metadata.category,
                    tagline: metadata.tagline,
                    developers: metadata.developers,
                    description: metadata.description,
                    dependencies: metadata.dependencies,
                    has_permissions: metadata.app_yml_jinja_permissions,
                    repo: metadata.repo,
                    support: metadata.support,
                    gallery: metadata.gallery,
                    path: metadata.path,
                    default_username: metadata.default_username,
                    default_password: metadata.default_password,
                    tor_only: metadata.tor_only,
                    update_containers: metadata.update_containers,
                    implements: metadata.implements,
                    version_control: metadata.version_control,
                    // This is only metadata for an app that's not installable, so compatible can never be true
                    compatible: false,
                    release_notes: metadata.release_notes,
                    port: 0,
                    internal_port: 0,
                    supports_https: false,
                }
            }
        }
    }
}
