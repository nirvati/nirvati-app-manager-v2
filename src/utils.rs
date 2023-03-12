use lazy_static::lazy_static;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

lazy_static! {
    // This should have been the following regex originally: \$(\{.*?}|[A-z1-9]+)
    // However, it lead to a double match of ${VAR} and {VAR} getting matched for some reason
    static ref ENV_VAR_REGEX: Regex = Regex::new(r"\$\{.*?}|\$[A-z1-9]+").unwrap();
}

// A helper for skipping deserialization of values that default to false
#[inline]
pub fn is_false(v: &bool) -> bool {
    !*v
}

/// A type that can be serialized into a string, but can also be various other types
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum StringLike {
    String(String),
    Int(i64),
    Bool(bool),
    Float(f64),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum StringOrNumber {
    String(String),
    Int(i64),
    Float(f64),
}

pub fn find_env_vars(string: &str) -> Vec<&str> {
    let mut result: Vec<&str> = Vec::new();
    let matches = ENV_VAR_REGEX.captures_iter(string);
    for captures in matches {
        for element in captures.iter().flatten() {
            let matched = element.as_str();
            // If the env var starts with ${, remove it and the closing }
            // Otherwise, just remove the $
            if matched.starts_with("${") {
                let simplified = &matched[2..matched.len() - 1];
                // Split it at :-, : or -, depending on which of these exist
                let split = simplified.splitn(2, '-').collect::<Vec<&str>>();
                let main_var = split[0].split(':').collect::<Vec<&str>>()[0];
                result.push(main_var);
                if split.len() > 1 {
                    let mut env_vars_in_default = find_env_vars(split[1]);
                    result.append(&mut env_vars_in_default);
                }
            } else {
                result.push(&matched[1..matched.len()]);
            };
        }
    }
    result
}
