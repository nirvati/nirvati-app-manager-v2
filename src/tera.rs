use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Result};
use tera::Tera;

use crate::{composegenerator::types::Permission, manage::files::get_app_settings};

mod builtins;
pub mod js;
pub mod second_stage;

#[allow(unused_must_use)]
pub fn process_metadata_yml_jinja(
    file: PathBuf,
    installed_apps: &[String],
    available_permissions: &[String],
    nirvati_root: &Path,
) -> Result<()> {
    let app_id = file
        .parent()
        .ok_or_else(|| anyhow!("Failed to get parent dir"))?
        .file_name()
        .ok_or_else(|| anyhow!("Failed to get file name"))?
        .to_str()
        .ok_or_else(|| anyhow!("Failed to convert to str"))?;
    let contents = std::fs::read_to_string(&file)?;
    let out_file = file.with_extension("");
    let dir = file
        .parent()
        .ok_or_else(|| anyhow!("Failed to get parent dir"))?;

    let mut tera_ctx = tera::Context::new();
    tera_ctx.insert("installed_apps", &installed_apps);
    tera_ctx.insert("available_permissions", &available_permissions);

    let mut tera = Tera::default();
    tera.functions
        .remove("get_env")
        .expect("get_env was not available in Tera, the API may have changed");
    builtins::register_builtins(&mut tera, nirvati_root, app_id);
    let tera_dir = dir.join("_tera");
    let mut code = String::new();
    let mut functions = Vec::new();
    if tera_dir.is_dir() {
        (code, functions) = js::parse_tera_helpers(&dir.join("_tera"))?;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || -> Result<()> {
        // This may execute JS code, so we need to sandbox it
        extrasafe::SafetyContext::new()
            .enable(
                extrasafe::builtins::SystemIO::nothing()
                    .allow_stdout()
                    .allow_stderr(),
            )
            .unwrap()
            .apply_to_current_thread()?;

        let mut tera = js::declare_js_functions(tera, &code, &functions)?;
        let result = tera.render_str(&contents, &tera_ctx);
        tx.send(result)?;
        Ok(())
    });
    let rendered = rx.recv_timeout(Duration::from_secs(2));
    thread.join().unwrap()?;
    let rendered = rendered
        .ok()
        .ok_or_else(|| anyhow!("Rendering timed out!"))??;
    std::fs::write(out_file, rendered)?;
    Ok(())
}

pub fn process_metadata_yml_jinjas(
    nirvati_root: &Path,
    installed_apps: &[String],
    available_permissions: &[String],
) -> Result<()> {
    // Loop through all subdirs, and process all metadata.yml.jinja files
    for entry in std::fs::read_dir(nirvati_root.join("apps"))? {
        let entry = entry?;
        let path = entry.path();
        let metadata_yml = path.join("metadata.yml.jinja");
        if metadata_yml.is_file() {
            process_metadata_yml_jinja(
                metadata_yml,
                installed_apps,
                available_permissions,
                nirvati_root,
            )?;
        }
    }
    Ok(())
}

pub fn assign_permission(
    map: &mut serde_json::Map<String, serde_json::Value>,
    from_app: &str,
    permission: &Permission,
    permissions: &[Permission],
    handle_recursion: bool,
    handled_values: Option<Vec<String>>,
) -> Result<()> {
    for (key, value) in &permission.variables {
        if map.contains_key(key) {
            tracing::warn!("Duplicate variable in permissions of app {}", from_app);
        }
        // Insert returns None if the key was not present
        assert!(map.insert(key.to_owned(), value.to_owned()).is_none());
    }
    if handle_recursion {
        let mut handled_values = Rc::new(handled_values.unwrap_or_default());
        // Loop through permissions in permission.includes,
        // and assign them to the app_metadata_obj
        for perm in &permission.includes {
            if handled_values.contains(&perm.to_string()) {
                tracing::warn!("Recursive permission detected in app {}", from_app);
                continue;
            }
            Rc::get_mut(&mut handled_values)
                .unwrap()
                .push(perm.to_string());
            if let Some(perm) = permissions.iter().find(|p| p.id == *perm) {
                assign_permission(
                    map,
                    from_app,
                    perm,
                    permissions,
                    true,
                    Some((*handled_values).clone()),
                )?;
            } else {
                tracing::warn!("Permission {} not found in app {}", perm, from_app);
            }
        }
    }

    Ok(())
}

#[allow(unused_must_use)]
pub fn process_app_yml_jinja(
    file: PathBuf,
    permissions: &[String],
    installed_apps: &[String],
    available_permissions_list: &[String],
    available_permissions: &HashMap<String, Vec<Permission>>,
    nirvati_root: &Path,
) -> Result<()> {
    let app_id = file
        .parent()
        .ok_or_else(|| anyhow!("Failed to get parent dir"))?
        .file_name()
        .ok_or_else(|| anyhow!("Failed to get file name"))?
        .to_str()
        .ok_or_else(|| anyhow!("Failed to convert to str"))?;
    let contents = std::fs::read_to_string(&file)?;
    let out_file = file.with_extension("");
    let dir = file
        .parent()
        .ok_or_else(|| anyhow!("Failed to get parent dir"))?;

    let mut tera_ctx = tera::Context::new();
    if permissions.contains(&"apps".to_string()) {
        tera_ctx.insert("installed_apps", &installed_apps);
        tera_ctx.insert("available_permissions", &available_permissions_list);
    }

    let mut app_metadata_obj = Rc::new(serde_json::Map::new());

    let mut assign_permission = |app: &str, perm: &Permission, handle_includes: bool| {
        let app_metadata_obj = Rc::get_mut(&mut app_metadata_obj).unwrap();
        assign_permission(
            app_metadata_obj,
            app,
            perm,
            available_permissions.get(app).unwrap(),
            handle_includes,
            None,
        )
    };

    for (app, perms) in available_permissions.iter() {
        if permissions.contains(app) {
            for perm in perms {
                assign_permission(app, perm, false);
            }
        } else {
            for perm in perms {
                if permissions.contains(&format!("{}/{}", app, perm.id)) {
                    assign_permission(app, perm, true);
                }
            }
        }
    }

    tera_ctx.insert("app_metadata", &Rc::try_unwrap(app_metadata_obj).unwrap());

    if let Some(settings) = get_app_settings(nirvati_root, app_id)? {
        tera_ctx.insert("settings", &settings);
    }

    let mut tera = Tera::default();
    tera.functions
        .remove("get_env")
        .expect("get_env was not available in Tera, the API may have changed");
    builtins::register_builtins(&mut tera, nirvati_root, app_id);
    let tera_dir = dir.join("_tera");
    let mut code = String::new();
    let mut functions = Vec::new();
    if tera_dir.is_dir() {
        (code, functions) = js::parse_tera_helpers(&dir.join("_tera"))?;
    }

    let tera_ctx = Arc::new(tera_ctx);
    let ctx_arc_2 = Arc::clone(&tera_ctx);

    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || -> Result<()> {
        // This may execute JS code, so we need to sandbox it
        extrasafe::SafetyContext::new()
            .enable(
                extrasafe::builtins::SystemIO::nothing()
                    .allow_stdout()
                    .allow_stderr(),
            )
            .unwrap()
            .apply_to_current_thread()?;

        let mut tera = js::declare_js_functions(tera, &code, &functions)?;
        let result = tera.render_str(&contents, &ctx_arc_2);
        tx.send(result)?;
        Ok(())
    });
    let rendered = rx.recv_timeout(Duration::from_secs(2));
    thread.join().unwrap()?;
    let rendered = rendered
        .ok()
        .ok_or_else(|| anyhow!("Rendering timed out!"))??;
    #[cfg(debug_assertions)]
    {
        let out_file = file.with_extension("stage1");
        std::fs::write(out_file, &rendered)?;
    }
    let mut available_files: Vec<PathBuf> = Vec::new();
    for perm in permissions {
        let split = perm.split('/').collect::<Vec<&str>>();
        if split.len() >= 2 {
            let app = split[0];
            let perm = split[1];
            if let Some(perm) = available_permissions
                .get(app)
                .unwrap()
                .iter()
                .find(|p| p.id == perm)
            {
                for dir in &perm.files {
                    available_files.push(nirvati_root.join("app-data").join(app).join(dir));
                }
            }
        } else {
            debug_assert!(split.len() == 1);
            let app = split[0];
            available_files.push(nirvati_root.join("app-data").join(app));
        }
    }
    let mut tera = second_stage::get_tera(nirvati_root.to_path_buf(), available_files);
    let rendered = tera.render_str(&rendered, &tera_ctx)?;
    std::fs::write(out_file, rendered)?;
    Ok(())
}
