use std::path::Path;

use crate::dependencies::{sort_deps, Node};
use anyhow::{anyhow, Result};

pub mod files;
pub mod ports;
pub mod processing;

pub fn determine_jinja_processing_order(
    nirvati_dir: &Path,
    installed_apps: &[String],
) -> Result<Vec<String>> {
    // Loop through all subdirs that contain a metadata.yml file
    // For each of them, read the metadata.yml file
    // And add it to the list of nodes
    let mut nodes = Vec::new();
    for entry in std::fs::read_dir(nirvati_dir.join("apps"))? {
        let entry = entry?;
        let path = entry.path();
        // If no app.yml.jinja exists, preprocessing isn't necessary,
        // So don't even load the app
        if path.is_dir()
            && path.join("app.yml.jinja").exists()
            && path.join("metadata.yml").exists()
        {
            let app_id = entry
                .file_name()
                .to_str()
                .ok_or_else(|| anyhow!("Failed to convert dir name into string!"))?
                .to_owned();
            let metadata = files::read_metadata_yml(nirvati_dir, &app_id)?;
            // Non-installed apps that require settings can't be processed yet
            if files::app_requires_settings(nirvati_dir, &app_id)
                && !installed_apps.contains(&app_id)
            {
                continue;
            }
            nodes.push(Node {
                // We can bail here because this should have been validated during repo sync
                id: app_id.to_owned(),
                dependencies: metadata
                    .into_app_yml_jinja_permissions()
                    .into_iter()
                    .map(|perm| perm.split('/').next().unwrap().to_string())
                    .collect(),
            });
        }
    }
    Ok(sort_deps(
        nodes
            .into_iter()
            .filter(|node| {
                // Ensure all dependencies are installed
                node.dependencies
                    .iter()
                    .all(|dep| installed_apps.contains(dep))
            })
            .collect::<Vec<_>>(),
    ))
}

pub fn determine_jinja_config_processing_order(
    nirvati_dir: &Path,
    installed_apps: &[String],
) -> Result<Vec<String>> {
    // Loop through all subdirs that contain a metadata.yml file
    // For each of them, read the metadata.yml file
    // And add it to the list of nodes
    let mut nodes = Vec::new();
    for entry in std::fs::read_dir(nirvati_dir.join("apps"))? {
        let entry = entry?;
        let path = entry.path();
        let app_yml = path.join("app.yml");
        let app_id = entry
            .file_name()
            .to_str()
            .ok_or_else(|| anyhow!("Failed to convert dir name into string!"))?
            .to_owned();
        if path.is_dir() && app_yml.exists() {
            let app_yml = files::read_app_yml(nirvati_dir, &app_id)?;
            nodes.push(Node {
                // We can bail here because this should have been validated during repo sync
                id: app_id.to_owned(),
                dependencies: app_yml
                    .into_config_jinja_permissions()
                    .into_iter()
                    .map(|perm| perm.split('/').next().unwrap().to_string())
                    .collect(),
            });
        }
    }
    Ok(sort_deps(
        nodes
            .into_iter()
            .filter(|node| {
                // Ensure all dependencies are installed
                node.dependencies
                    .iter()
                    .all(|dep| installed_apps.contains(dep))
            })
            .collect::<Vec<_>>(),
    ))
}
