use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::composegenerator::types::{Command, Dependency, Permission};
use crate::manage::ports::{PortMapEntry, PortPriority};
use crate::utils::{is_false, StringLike, StringOrNumber};

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, JsonSchema)]
pub struct PortsDefinition {
    /// Ports that may not be proxied through Caddy
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub direct_tcp: HashMap<u16, u16>,
    /// TCP ports that may be proxied through Caddy (and support TLS)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tcp: HashMap<u16, u16>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub http: HashMap<u16, u16>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub udp: HashMap<u16, u16>,
}

impl PortsDefinition {
    pub fn is_empty(&self) -> bool {
        self.direct_tcp.is_empty()
            && self.tcp.is_empty()
            && self.http.is_empty()
            && self.udp.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum StringOrMap {
    String(String),
    Map(BTreeMap<String, String>),
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
pub struct Container {
    // These can be copied directly without validation
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_grace_period: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_hosts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shm_size: Option<StringOrNumber>,
    // These need security checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub environment: BTreeMap<String, StringLike>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub cap_add: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_mode: Option<String>,
    // These are not directly present in a compose file and need to be converted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_priority: Option<PortPriority>,
    #[serde(skip_serializing_if = "PortsDefinition::is_empty", default)]
    pub required_ports: PortsDefinition,
    #[serde(
        skip_serializing_if = "BTreeMap::<String, StringOrMap>::is_empty",
        default
    )]
    pub mounts: BTreeMap<String, StringOrMap>,
    #[serde(default = "bool::default")]
    #[serde(skip_serializing_if = "is_false")]
    /// Set this to true to make Caddy proxy any traffic on the TCP layer directly instead of handling HTTP
    pub direct_tcp: bool,
    #[serde(default = "bool::default")]
    #[serde(skip_serializing_if = "is_false")]
    pub disable_caddy: bool,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, JsonSchema)]
pub struct InputMetadata {
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
    /// Other apps this app depends on
    pub dependencies: Vec<Dependency>,
    /// App repository name -> repo URL
    pub repo: BTreeMap<String, String>,
    /// A support link for the app
    pub support: String,
    /// A list of promo images for the apps
    pub gallery: Option<Vec<String>>,
    /// The path the "Open" link on the dashboard should lead to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The app's default username
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_username: Option<String>,
    /// The app's default password.
    pub default_password: Option<String>,
    #[serde(default = "bool::default")]
    #[serde(skip_serializing_if = "is_false")]
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
    #[serde(default, skip_serializing_if = "BTreeMap::<String, String>::is_empty")]
    pub release_notes: BTreeMap<String, String>,
    /// A directory any app with full permissions to this app can access
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_dir: Option<String>,
    /// Permissions this app's app.yml.jinja has
    #[serde(
        default = "Vec::default",
        skip_serializing_if = "Vec::<String>::is_empty"
    )]
    pub app_yml_jinja_permissions: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, JsonSchema)]
pub struct AppYmlMetadata {
    /// Permissions this app exposes
    #[serde(
        default = "Vec::default",
        skip_serializing_if = "Vec::<Permission>::is_empty"
    )]
    pub permissions: Vec<Permission>,
    /// Permissions this app's config Jinja files have
    #[serde(
        default = "Vec::default",
        skip_serializing_if = "Vec::<String>::is_empty"
    )]
    pub jinja_config_permissions: Vec<String>,
    /// Permissions this app has
    #[serde(
        default = "Vec::default",
        skip_serializing_if = "Vec::<String>::is_empty"
    )]
    pub has_permissions: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
/// Nirvati app definition
pub struct AppYml {
    pub version: u8,
    pub services: HashMap<String, Container>,
    pub metadata: AppYmlMetadata,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
/// Nirvati app metadata definition
pub struct MetadataYml {
    pub version: u8,
    pub metadata: InputMetadata,
}

impl AppYml {
    pub fn get_ports(&self, own_id: &str, implements: Option<String>) -> Vec<PortMapEntry> {
        let mut ports = Vec::new();
        for (container_name, container) in self.services.iter() {
            if let Some(port) = container.port {
                ports.push(PortMapEntry {
                    app: own_id.to_owned(),
                    internal_port: port,
                    public_port: port,
                    container: container_name.to_owned(),
                    implements: implements.clone(),
                    priority: container.port_priority.unwrap_or(PortPriority::Optional),
                });
            }
            for (public_port, container_port) in container.required_ports.direct_tcp.iter() {
                ports.push(PortMapEntry {
                    app: own_id.to_owned(),
                    internal_port: *container_port,
                    public_port: *public_port,
                    container: container_name.to_owned(),
                    implements: implements.clone(),
                    priority: PortPriority::Required,
                });
            }
            for (public_port, container_port) in container.required_ports.tcp.iter() {
                ports.push(PortMapEntry {
                    app: own_id.to_owned(),
                    internal_port: *container_port,
                    public_port: *public_port,
                    container: container_name.to_owned(),
                    implements: implements.clone(),
                    priority: PortPriority::Required,
                });
            }
            for (public_port, container_port) in container.required_ports.udp.iter() {
                if ports.iter().any(|p| p.public_port == *public_port) {
                    continue;
                }
                ports.push(PortMapEntry {
                    app: own_id.to_owned(),
                    internal_port: *container_port,
                    public_port: *public_port,
                    container: container_name.to_owned(),
                    implements: implements.clone(),
                    priority: PortPriority::Required,
                });
            }
            for (public_port, container_port) in container.required_ports.http.iter() {
                if ports.iter().any(|p| p.public_port == *public_port) {
                    continue;
                }
                ports.push(PortMapEntry {
                    app: own_id.to_owned(),
                    internal_port: *container_port,
                    public_port: *public_port,
                    container: container_name.to_owned(),
                    implements: implements.clone(),
                    priority: PortPriority::Required,
                });
            }
        }
        ports
    }
}
