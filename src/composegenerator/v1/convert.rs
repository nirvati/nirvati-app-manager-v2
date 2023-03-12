use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};

use super::{
    helpers::find_permission_that_matches,
    types::{AppYml, Container, InputMetadata as Metadata, StringOrMap},
};
use crate::{
    composegenerator::{
        output::types::Service,
        types::{CaddyEntry, OutputMetadata, Permission, ResultYml},
    },
    manage::ports::PortMapEntry,
    utils::{find_env_vars, StringLike},
};

static ALLOWED_ENV_VARS: [&str; 3] = ["API_IP", "DEVICE_HOSTNAME", "DEVICE_IP"];

macro_rules! require_permission_metadata {
    ($metadata:ident, $perm_name:expr) => {
        if !$metadata.has_permissions.contains(&$perm_name.to_owned()) {
            $metadata.has_permissions.push($perm_name.to_owned());
        }
    };
}

macro_rules! require_permission {
    ($result:ident, $perm_name:expr) => {
        if !$result
            .metadata
            .has_permissions
            .contains(&$perm_name.to_owned())
        {
            $result.metadata.has_permissions.push($perm_name.to_owned());
        }
    };
}

fn validate_env_access(
    result: &mut ResultYml,
    available_permissions: &HashMap<String, Vec<Permission>>,
) {
    let mut accessed_env_vars = Vec::new();
    for service in result.spec.services.values() {
        let env_vars_in_cmd = service
            .command
            .as_ref()
            .map(|cmd| cmd.get_env_vars())
            .unwrap_or_default();
        accessed_env_vars.extend(env_vars_in_cmd);
        let env_vars_in_entrypoint = service
            .entrypoint
            .as_ref()
            .map(|cmd| cmd.get_env_vars())
            .unwrap_or_default();
        accessed_env_vars.extend(env_vars_in_entrypoint);
        for value in service.environment.values() {
            if let StringLike::String(value) = value {
                accessed_env_vars.extend(find_env_vars(value));
            }
        }
    }
    for env_var in accessed_env_vars {
        if !ALLOWED_ENV_VARS.contains(&env_var) {
            if env_var.starts_with("APP_") {
                let mut split = env_var.split('_');
                if split.next() != Some("APP") {
                    unreachable!();
                }
                let Some(app_name) = split.next() else {
                    require_permission!(result, "root");
                    continue;
                };
                // Because next() is called twice, the iterator is at different elements for the first and second check
                if split.next().is_none() || split.next().is_some() {
                    require_permission!(result, "root");
                } else {
                    let app_permissions = available_permissions
                        .get(app_name)
                        .cloned()
                        .unwrap_or_default();
                    let ideal_permission = find_permission_that_matches(
                        app_name,
                        &app_permissions,
                        &result.metadata.has_permissions,
                        |perm| {
                            perm.variables.iter().any(|(name, value)| {
                                name == env_var
                                    && (value.as_str() == Some(&format!("${}", env_var))
                                        || value.as_str() == Some(&format!("${{{}}}", env_var)))
                            })
                        },
                    );
                    if let Some(permission) = ideal_permission {
                        require_permission!(result, format!("{}/{}", app_name, permission.id));
                    } else {
                        require_permission!(result, app_name);
                    }
                }
            } else {
                require_permission!(result, "root");
            }
        }
    }
}

pub fn convert_mounts(
    result: &mut Service,
    input_service: &Container,
    metadata: &mut OutputMetadata,
    available_permissions: &HashMap<String, Vec<Permission>>,
) -> Result<()> {
    for (mount_name, target) in &input_service.mounts {
        match (mount_name.as_str(), target) {
            ("data", StringOrMap::Map(map)) => {
                for (host_dir, container_dir) in map {
                    if host_dir.contains(':')
                        || host_dir.contains("..")
                        || container_dir.contains(':')
                        || container_dir.contains("..")
                        || !find_env_vars(host_dir).is_empty()
                        || !find_env_vars(container_dir).is_empty()
                    {
                        tracing::warn!("Invalid mount name: {}", mount_name);
                        continue;
                    }
                    result
                        .volumes
                        .push(format!("${{APP_DATA_DIR}}/{}:{}", host_dir, container_dir));
                }
            }
            (mount_name, StringOrMap::String(str)) => {
                if str.contains(':')
                    || str.contains("..")
                    || mount_name.contains(':')
                    || mount_name.contains("..")
                {
                    tracing::warn!("Invalid mount name: {}", mount_name);
                    continue;
                }
                match mount_name {
                    "jwt-pubkey" => {
                        result.volumes.push(format!("${{JWT_PUBKEY}}:{}", str));
                    }
                    mount_name => {
                        let split = mount_name.split('/').collect::<Vec<_>>();
                        if split.len() > 2 {
                            tracing::warn!("Invalid mount name: {}", mount_name);
                            continue;
                        } else if split.len() == 2 {
                            let app_name = split[0];
                            let mount_name = split[1];
                            let app_permissions = available_permissions
                                .get(app_name)
                                .cloned()
                                .unwrap_or_default();
                            let ideal_permission = find_permission_that_matches(
                                app_name,
                                &app_permissions,
                                &metadata.has_permissions,
                                |perm| perm.files.iter().any(|name| name == mount_name),
                            );
                            result.volumes.push(format!(
                                "${{APPS_DATA_DIR}}/{}/{}:{}",
                                app_name, mount_name, str
                            ));
                            if let Some(permission) = ideal_permission {
                                require_permission_metadata!(
                                    metadata,
                                    format!("{}/{}", app_name, permission.id)
                                );
                            } else {
                                require_permission_metadata!(metadata, app_name);
                            }
                        } else {
                            result
                                .volumes
                                .push(format!("${{APPS_DATA_DIR}}/{}:{}", mount_name, str));
                            require_permission_metadata!(metadata, mount_name);
                        }
                    }
                }
            }
            _ => {
                tracing::warn!(
                    "Failed to parse mount {}: {:?} of app {}",
                    mount_name,
                    target,
                    metadata.id
                );
            }
        }
    }
    Ok(())
}

fn handle_ports(
    service_name: &str,
    result: &mut Service,
    input_service: &Container,
    port_map: &[PortMapEntry],
) -> Result<Vec<CaddyEntry>> {
    let mut new_caddy_entries = Vec::new();
    if service_name == "main" {
        let main_port = input_service
            .port
            .ok_or_else(|| anyhow!("No main port found!"))?;
        let port_map_entry = port_map
            .iter()
            .find(|port| port.internal_port == main_port && port.container == service_name)
            .ok_or_else(|| anyhow!("No port map entry found for port {}", main_port))?;
        if input_service.disable_caddy {
            result
                .ports
                .push(format!("{}:{}", port_map_entry.public_port, main_port));
        } else {
            new_caddy_entries.push(CaddyEntry {
                public_port: port_map_entry.public_port,
                internal_port: main_port,
                container_name: service_name.to_string(),
                is_primary: true,
                is_l4: input_service.direct_tcp,
            });
        }
    }
    for (public_port, internal_port) in &input_service.required_ports.http {
        // Just a check, this should always be validated before
        assert!(port_map
            .iter()
            .any(|port| port.internal_port == *internal_port && port.container == service_name));
        new_caddy_entries.push(CaddyEntry {
            public_port: *public_port,
            internal_port: *internal_port,
            container_name: service_name.to_string(),
            is_primary: false,
            is_l4: false,
        });
    }
    for (public_port, internal_port) in &input_service.required_ports.tcp {
        // Just a check, this should always be validated before
        assert!(port_map
            .iter()
            .any(|port| port.internal_port == *internal_port && port.container == service_name));
        new_caddy_entries.push(CaddyEntry {
            public_port: *public_port,
            internal_port: *internal_port,
            container_name: service_name.to_string(),
            is_primary: false,
            is_l4: true,
        });
    }
    for (public_port, internal_port) in &input_service.required_ports.direct_tcp {
        // Just a check, this should always be validated before
        assert!(port_map
            .iter()
            .any(|port| port.internal_port == *internal_port && port.container == service_name));
        result
            .ports
            .push(format!("{}:{}", public_port, internal_port));
    }
    for (public_port, internal_port) in &input_service.required_ports.udp {
        // Just a check, this should always be validated before
        assert!(port_map
            .iter()
            .any(|port| port.internal_port == *internal_port && port.container == service_name));
        result
            .ports
            .push(format!("{}:{}/udp", public_port, internal_port));
    }

    Ok(new_caddy_entries)
}

pub fn convert_app_yml(
    app_id: &str,
    app_yml: &AppYml,
    metadata: Metadata,
    port_map: &[PortMapEntry],
    available_permissions: &HashMap<String, Vec<Permission>>,
) -> Result<ResultYml> {
    let mut result = ResultYml::default();
    let main_port;
    let main_port_public;
    let supports_https;
    {
        let main_container = app_yml
            .services
            .get("main")
            .ok_or_else(|| anyhow!("No main container found!"))?;
        main_port = main_container
            .port
            .ok_or_else(|| anyhow!("No main port found!"))?;
        main_port_public = port_map
            .iter()
            .find(|port| port.internal_port == main_port)
            .ok_or_else(|| anyhow!("No main port found!"))?
            .public_port;
        supports_https = !main_container.direct_tcp;
    }
    result.metadata = OutputMetadata {
        id: app_id.to_owned(),
        name: metadata.name,
        version: metadata.version,
        category: metadata.category,
        tagline: metadata.tagline,
        developers: metadata.developers,
        description: metadata.description,
        dependencies: metadata.dependencies,
        has_permissions: metadata.app_yml_jinja_permissions,
        repo: metadata.repo,
        support: metadata.support,
        gallery: metadata.gallery,
        path: metadata.path,
        default_username: metadata.default_username,
        default_password: metadata.default_password,
        tor_only: metadata.tor_only,
        update_containers: metadata.update_containers,
        implements: metadata.implements,
        version_control: metadata.version_control,
        // This is only metadata for an app that's compatible
        compatible: true,
        release_notes: metadata.release_notes,
        port: main_port_public,
        internal_port: main_port,
        supports_https,
    };
    for (service_id, service) in &app_yml.services {
        // These properties need no validation
        let mut result_service = Service {
            image: service.image.clone(),
            restart: service.restart.clone(),
            stop_grace_period: service.stop_grace_period.clone(),
            stop_signal: service.stop_signal.clone(),
            user: service.user.clone(),
            init: service.init,
            depends_on: service.depends_on.clone(),
            extra_hosts: service.extra_hosts.clone(),
            working_dir: service.working_dir.clone(),
            shm_size: service.shm_size.clone(),
            network_mode: service.network_mode.clone(),
            ports: Vec::new(),
            volumes: Vec::new(),
            cap_add: service.cap_add.clone(),
            command: service.command.clone(),
            entrypoint: service.entrypoint.clone(),
            environment: service.environment.clone(),
            ..Default::default()
        };
        if let Some(network_mode) = &service.network_mode {
            if network_mode == "host" {
                require_permission!(result, "network");
            } else {
                bail!("Unsupported network_mode!");
            }
        }

        for capability in &service.cap_add {
            match capability.as_str() {
                "CAP_NET_RAW" => {
                    require_permission!(result, "network");
                }
                _ => {
                    require_permission!(result, "root");
                }
            }
        }

        convert_mounts(
            &mut result_service,
            &service,
            &mut result.metadata,
            available_permissions,
        )?;

        let mut new_caddy_entries =
            handle_ports(&service_id, &mut result_service, &service, port_map)?;
        result.caddy_entries.append(&mut new_caddy_entries);
        result
            .spec
            .services
            .insert(service_id.to_owned(), result_service);
    }
    validate_env_access(&mut result, available_permissions);
    Ok(result)
}
