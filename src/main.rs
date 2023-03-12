#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::Result;
use clap::{Parser, Subcommand};
use manage::files::get_all_metadata_ymls;
use serde::{Deserialize, Serialize};

use crate::composegenerator::v1::RESERVED_NAMES;

mod composegenerator;
mod dependencies;
mod manage;
mod repos;
mod tera;
pub(crate) mod utils;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generates docker-compose.yml files
    Generate { dir: String },
    /// Installs an app
    Install {
        dir: String,
        app: String,
        #[clap(long)]
        settings: Option<String>,
    },
    AttemptInstall {
        dir: String,
        app: String,
        #[clap(long)]
        settings: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppInstallState {
    success: bool,
    has_permissions: Vec<String>,
    other_app_permission_additions: HashMap<String, Vec<String>>,
}

fn handle_cmd(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Generate { dir } => {
            let dir = std::path::Path::new(&dir);
            let apps_dir = dir.join("apps");
            let installed_apps = manage::files::get_installed_apps(dir)?;
            let mut available_permissions = installed_apps
                .iter()
                .flat_map(|app| {
                    // Apps can only be installed if they have an app.yml, so assume app.yml files exist for installed apps
                    let app_yml = manage::files::read_app_yml(&apps_dir, app);
                    let Ok(app_yml) = app_yml else {
                        return vec![app.to_owned()];
                    };
                    let mut permissions = app_yml
                        .into_exported_permissions()
                        .into_iter()
                        .map(|elem| format!("{}/{}", app, elem.id))
                        .collect::<Vec<_>>();
                    permissions.push(app.to_owned());
                    permissions
                })
                .collect::<Vec<_>>();
            let mut builtin_permissions = RESERVED_NAMES
                .iter()
                .map(|elem| elem.to_string())
                .collect::<Vec<_>>();
            available_permissions.append(&mut builtin_permissions);
            tera::process_metadata_yml_jinjas(dir, &installed_apps, &available_permissions)?;
            {
                let registry = get_all_metadata_ymls(dir)?;
                let registry_file = dir.join("apps").join("registry.json");
                let registry_file = std::fs::File::create(registry_file)?;
                serde_json::to_writer_pretty(registry_file, &registry)?;
            }
            let apps = manage::determine_jinja_processing_order(dir, &installed_apps)?;
            let permission_map = HashMap::from_iter(installed_apps.iter().filter_map(|app| {
                // Apps can only be installed if they have an app.yml, so assume app.yml files exist for installed apps
                match manage::files::read_app_yml(dir, app) {
                    Err(err) => {
                        tracing::warn!("Failed to read app.yml for app {}: {:#}", app, err);
                        None
                    }
                    Ok(app_yml) => Some((app.to_owned(), app_yml.into_exported_permissions())),
                }
            }));
            manage::processing::process_app_ymls(dir, &apps, permission_map)?;
        }
        Commands::Install { dir, app, settings } => {
            // We don't interact with Docker here, the host scripts do that
            let nirvati_dir = std::path::Path::new(&dir);
            let app_dir = nirvati_dir.join("apps").join(&app);
            if !app_dir.exists() {
                return Err(anyhow::anyhow!("App does not exist"));
            }
            if let Some(settings) = settings {
                let settings = serde_json::from_str(&settings)?;
                manage::files::save_app_settings(&app, settings, nirvati_dir)?;
            }
            handle_cmd(Commands::Generate { dir: dir.clone() })?;
            manage::files::add_installed_app(&app, nirvati_dir)?;
            // Do another generate pass to ensure all apps that depend on this app also have their config regenerated
            if let Err(msg) = handle_cmd(Commands::Generate { dir: dir.clone() }) {
                tracing::error!("Failed to generate: {:#}", msg);
                manage::files::remove_installed_app(&app, nirvati_dir)?;
            }
        }
        Commands::AttemptInstall { dir, app, settings } => {
            let nirvati_dir = std::path::Path::new(&dir);
            let app_dir = nirvati_dir.join("apps").join(&app);
            let state_yml = nirvati_dir.join("apps").join(&app).join("state.yml");
            let state_yml = std::fs::File::create(state_yml)?;
            if !app_dir.exists() {
                return Err(anyhow::anyhow!("App does not exist"));
            }
            if let Some(settings) = settings {
                let settings = serde_json::from_str(&settings)?;
                manage::files::save_app_settings(&app, settings, nirvati_dir)?;
            }
            // First, load the current registry.json
            let registry = manage::files::get_app_registry(nirvati_dir)?;
            if let Err(err) = handle_cmd(Commands::Generate { dir: dir.clone() }) {
                let state = AppInstallState {
                    success: false,
                    has_permissions: vec![],
                    other_app_permission_additions: HashMap::new(),
                };
                serde_yaml::to_writer(state_yml, &state)?;
                return Err(err);
            };
            manage::files::add_installed_app(&app, nirvati_dir)?;
            // Do another generate pass to ensure all apps that depend on this app also have their config regenerated
            if let Err(err) = handle_cmd(Commands::Generate { dir: dir.clone() }) {
                manage::files::remove_installed_app(&app, nirvati_dir)?;
                let state = AppInstallState {
                    success: false,
                    has_permissions: vec![],
                    other_app_permission_additions: HashMap::new(),
                };
                serde_yaml::to_writer(state_yml, &state)?;
                return Err(err);
            }
            let new_registry = manage::files::get_app_registry(nirvati_dir)?;
            let registry_map: HashMap<
                String,
                &composegenerator::types::OutputMetadata,
                std::collections::hash_map::RandomState,
            > = HashMap::from_iter(registry.iter().map(|app| (app.id.clone(), app)));
            let new_registry_map: HashMap<
                String,
                &composegenerator::types::OutputMetadata,
                std::collections::hash_map::RandomState,
            > = HashMap::from_iter(new_registry.iter().map(|app| (app.id.clone(), app)));
            let other_app_permission_additions: HashMap<
                String,
                Vec<String>,
                std::collections::hash_map::RandomState,
            > = HashMap::from_iter(registry_map.into_iter().filter_map(|(app, app_info)| {
                if let Some(new_app_info) = new_registry_map.get(&app) {
                    if app_info.has_permissions != new_app_info.has_permissions {
                        let added_permissions = new_app_info
                            .has_permissions
                            .iter()
                            .filter_map(|elem| {
                                if !app_info.has_permissions.contains(elem) {
                                    Some(elem.to_owned())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        Some((app.clone(), added_permissions))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }));
            if let Some(new_app) = new_registry_map.get(&app) {
                let state = AppInstallState {
                    success: true,
                    has_permissions: new_app.has_permissions.clone(),
                    other_app_permission_additions,
                };
                serde_yaml::to_writer(state_yml, &state)?;
            } else {
                let state = AppInstallState {
                    success: false,
                    has_permissions: vec![],
                    other_app_permission_additions: HashMap::new(),
                };
                serde_yaml::to_writer(state_yml, &state).expect("Writing failed!");
            }
            manage::files::remove_installed_app(&app, nirvati_dir).expect("Removing app failed!");
            // Restore the old registry.json
            manage::files::write_app_registry( nirvati_dir, &registry)?;
            // Do another generate pass to ensure all changes have been reverted
            if let Err(msg) = handle_cmd(Commands::Generate { dir: dir.clone() }) {
                tracing::error!("Failed to generate: {:#}", msg);
                manage::files::remove_installed_app(&app, nirvati_dir)?;
            }
        }
    }
    Ok(())
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    handle_cmd(cli.command).expect("An error occurred!");
}
