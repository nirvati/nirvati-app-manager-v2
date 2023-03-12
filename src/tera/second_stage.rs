// A minimal processor that doesn't include many tools (Most notably, no JS), but does support reading UTF-8 text files
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tera::Tera;

use crate::manage::files::set_next_app_regenerate;

pub fn get_tera(nirvati_root: PathBuf, can_read_files: Vec<PathBuf>) -> Tera {
    let mut tera = Tera::default();
    tera.functions
        .remove("get_env")
        .expect("get_env was not available in Tera, the API may have changed");
    let nirvati_root = Arc::new(nirvati_root);
    let nirvati_root_clone = Arc::clone(&nirvati_root);
    tera.register_function(
        "read_file",
        move |args: &HashMap<String, serde_json::Value>| {
            let path = args
                .get("path")
                .ok_or_else(|| tera::Error::msg("Missing path argument"))?
                .as_str()
                .ok_or_else(|| tera::Error::msg("Path argument is not a string"))?;
            let path = nirvati_root.join(path);
            // Check if can_read_files includes path or any of its parents
            let mut check_path = path.clone();
            let mut found = false;
            while &check_path != nirvati_root.as_ref() {
                if can_read_files.iter().any(|p| p == &path) {
                    found = true;
                    break;
                }
                check_path = path.parent().unwrap().to_path_buf();
            }
            if !found {
                return Err(tera::Error::msg(format!(
                    "Path {} is not in can_read_files",
                    path.display()
                )));
            }
            let contents = std::fs::read_to_string(&path).or_else(|_| {
                // if args.fallback is set, return that instead of an error
                if let Some(fallback) = args.get("fallback") {
                    Ok(fallback
                        .as_str()
                        .ok_or(tera::Error::msg("Fallback is not a string"))?
                        .to_owned())
                } else {
                    Err(tera::Error::msg(format!(
                        "Failed to read file {}",
                        path.display()
                    )))
                }
            })?;
            Ok(tera::Value::String(contents))
        },
    );
    tera.register_function(
        "require_regen",
        move |args: &HashMap<String, serde_json::Value>| {
            let delay_in_s = args
                .get("delay_in_s")
                .ok_or_else(|| tera::Error::msg("Missing delay_in_s argument"))?
                .as_u64()
                .ok_or_else(|| tera::Error::msg("delay_in_s argument is not a number"))?;
            let regen_time = std::time::SystemTime::now()
                .checked_add(std::time::Duration::from_secs(delay_in_s))
                .ok_or_else(|| tera::Error::msg("Delay is too large"))?;
            // Require a minimum delay of 1 minute
            if delay_in_s < 60 {
                return Err(tera::Error::msg("Delay is too small"));
            }
            let regen_unix_timestamp = regen_time
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| tera::Error::msg("Delay is too large"))?
                .as_secs();
            set_next_app_regenerate(&nirvati_root_clone, regen_unix_timestamp)
                .map_err(|_| tera::Error::msg("Failed to set next app regenerate time"))?;
            Ok(tera::Value::String("".to_owned()))
        },
    );
    tera
}
