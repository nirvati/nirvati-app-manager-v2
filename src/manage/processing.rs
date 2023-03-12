use std::{collections::HashMap, path::Path};

use crate::{composegenerator::types::Permission, tera::process_app_yml_jinja};

use super::{
    files::{read_app_yml, read_metadata_yml},
    ports::resolve_port_conflicts,
};

pub fn process_app_ymls(
    nirvati_root: &Path,
    sorted_apps: &[String],
    mut available_permissions: HashMap<String, Vec<Permission>>,
) -> anyhow::Result<()> {
    let installed_apps = super::files::get_installed_apps(nirvati_root)?;
    let apps_dir = nirvati_root.join("apps");
    let mut new_registry_entries = Vec::new();
    let mut available_permissions_strings = available_permissions
        .iter()
        .flat_map(|(k, v)| {
            let mut permissions = v
                .iter()
                .map(|p| format!("{}/{}", k, p.id))
                .collect::<Vec<_>>();
            permissions.push(k.to_owned());
            permissions
        })
        .collect::<Vec<_>>();
    let mut all_ports = Vec::new();
    for app in sorted_apps {
        let app_dir = apps_dir.join(app);
        let Ok(metadata) = read_metadata_yml(&nirvati_root, app) else {
            tracing::warn!("Failed to read metadata for app {}", app);
            continue
        };
        let app_yml_jinja = app_dir.join("app.yml.jinja");
        if app_yml_jinja.exists() {
            if let Err(err) = process_app_yml_jinja(
                app_yml_jinja,
                metadata.get_app_yml_jinja_permissions(),
                &installed_apps,
                &available_permissions_strings,
                &available_permissions,
                nirvati_root,
            ) {
                tracing::error!("Failed to process app.yml.jinja for app {}: {:#}", app, err);
                continue;
            }
        }
        let app_yml = app_dir.join("app.yml");
        if app_yml.exists() {
            let app_yml = read_app_yml(&nirvati_root, app)?;
            let mut ports = app_yml.get_ports(
                app,
                metadata
                    .get_basic_output_metadata(app.to_string())
                    .implements,
            );
            all_ports.append(&mut ports);
            let app_available_permissions = app_yml.into_exported_permissions();
            available_permissions.insert(app.to_owned(), app_available_permissions.clone());
            if installed_apps.contains(app) {
                if let Some(implements) = metadata
                    .get_basic_output_metadata(app.to_owned())
                    .implements
                {
                    available_permissions
                        .insert(implements.to_owned(), app_available_permissions.clone());
                }
            }

            available_permissions_strings.extend(
                app_available_permissions
                    .into_iter()
                    .map(|perm| format!("{}/{}", app, perm.id))
                    .collect::<Vec<_>>(),
            );
            available_permissions_strings.push(app.to_owned());
        } else {
            tracing::warn!("App {} does not have an app.yml", app);
        }
    }
    let (all_ports, apps_with_conflicts) = resolve_port_conflicts(all_ports, &installed_apps);
    let apps_to_convert = sorted_apps.iter().filter(|app| {
        let app_dir = apps_dir.join(app);
        let app_yml = app_dir.join("app.yml");
        app_yml.exists() && !apps_with_conflicts.contains(app)
    });
    for app in &apps_with_conflicts {
        tracing::warn!("App {} has conflicting ports", app);
    }
    for app in apps_to_convert {
        let app_dir = apps_dir.join(app);
        let app_yml = read_app_yml(&nirvati_root, app)?;
        let metadata = read_metadata_yml(&nirvati_root, app)?;
        // TODO: Once drain_filter is stable, use that here
        let app_ports = all_ports
            .iter()
            .filter(|port| &port.app == app)
            .map(|port| port.to_owned())
            .collect::<Vec<_>>();
        let result = app_yml.convert(app, &app_ports, metadata, &available_permissions);
        let Ok(result) = result else {
            tracing::error!("Failed to convert app.yml for app {}", app);
            tracing::error!("{:#}", result.unwrap_err());
            continue;
        };
        #[cfg(debug_assertions)]
        {
            let result_yml = app_dir.join("result.yml");
            let result_writer = std::fs::File::create(&result_yml)?;
            let mut result_writer = std::io::BufWriter::new(result_writer);
            serde_yaml::to_writer(&mut result_writer, &result)?;
        }
        new_registry_entries.push(result.metadata);
    }
    let current_registry = super::files::get_app_registry(nirvati_root)?;
    let new_app_ids = new_registry_entries
        .iter()
        .map(|entry| entry.id.to_owned())
        .collect::<Vec<_>>();
    let mut new_registry = current_registry;
    new_registry.retain(|entry| !new_app_ids.contains(&entry.id));
    new_registry.append(&mut new_registry_entries.clone());
    super::files::write_app_registry(nirvati_root, &new_registry)?;
    Ok(())
}
