use std::collections::HashMap;

use anyhow::Result;
use serde_json::{Map, Value};

use super::types::PortMapElement;

pub fn get_host_port(port_map: &[PortMapElement], internal_port: u16) -> Option<&PortMapElement> {
    return port_map
        .iter()
        .find(|&elem| elem.internal_port == internal_port);
}

pub fn validate_port_map_app(
    port_map_app: &Map<String, Value>,
) -> Result<HashMap<String, Vec<PortMapElement>>> {
    Ok(serde_json::from_value::<
        HashMap<String, Vec<PortMapElement>>,
    >(Value::Object(port_map_app.to_owned()))?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn validate_port_map_app() {
        let example_port_map = json!({
            "main": [
                {
                    "internalPort": 3000,
                    "publicPort": 3000,
                    "dynamic": true,
                }
            ]
        });
        let result = super::validate_port_map_app(example_port_map.as_object().unwrap());
        assert!(result.is_ok());
    }
}
