use std::{collections::HashMap, path::Path};

use anyhow::Result;
use hmac_sha256::HMAC;
use tera::Tera;

pub fn register_builtins(tera: &mut Tera, nirvati_root: &Path, app_id: &str) -> Result<()> {
    let nirvati_seed = nirvati_root.join("db").join("nirvati-seed").join("seed");
    let nirvati_seed = std::fs::read_to_string(nirvati_seed)?;
    let app_id = app_id.to_string();
    tera.register_function(
        "derive_entropy",
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let identifier = args
                .get("identifier")
                .ok_or_else(|| tera::Error::msg("identifier not provided"))?
                .as_str()
                .ok_or_else(|| tera::Error::msg("identifier is not a string"))?;
            let mut hasher = HMAC::new(&nirvati_seed);
            hasher.update(format!("{}:{}", app_id, identifier).as_bytes());
            let result = hasher.finalize();
            Ok(tera::Value::String(hex::encode(result)))
        },
    );
    // This can only be used during stage 2
    tera.register_function(
        "read_file",
        |_: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            Err(tera::Error::msg(
                "read_file needs to be in a {% raw %} block",
            ))
        },
    );
    Ok(())
}
