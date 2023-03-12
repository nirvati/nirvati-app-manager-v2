use crate::utils::{StringLike, StringOrNumber};

use super::super::types::Command;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq, Debug, JsonSchema)]
pub struct NetworkEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_address: Option<String>,
}

#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Debug, JsonSchema)]
#[serde(rename = "service")]
pub struct Service {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub cap_add: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Command>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, StringLike>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_hosts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<BTreeMap<String, NetworkEntry>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ports: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_grace_period: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub volumes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shm_size: Option<StringOrNumber>,
}

#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Debug, JsonSchema)]
#[serde(rename = "Compose Specification")]
pub struct ComposeSpecification {
    #[serde(default = "BTreeMap::default")]
    #[serde(skip_serializing_if = "BTreeMap::<String, Service>::is_empty")]
    pub services: BTreeMap<String, Service>,
}
