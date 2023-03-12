use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;

// A port map as used during creating the port map
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct PortMapEntry {
    pub app: String,
    // Internal port
    pub internal_port: u16,
    pub public_port: u16,
    pub container: String,
    pub implements: Option<String>,
    pub priority: PortPriority,
}

pub static RESERVED_PORTS: [u16; 2] = [
    80,  // HTTP
    443, // HTTPS
];

#[derive(
    Serialize_repr,
    Deserialize_repr,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    JsonSchema,
)]
#[repr(u8)]
pub enum PortPriority {
    /// Outside port doesn't matter
    #[default]
    Optional,
    /// Outside port is preferred, but not required for the app to work
    Recommended,
    /// Port is required for the app to work
    Required,
}

/// Returns (sorted_entries, apps_with_conflicts)
pub fn resolve_port_conflicts(
    mut entries: Vec<PortMapEntry>,
    installed_apps: &[String],
) -> (Vec<PortMapEntry>, Vec<String>) {
    // Resolve any conflicts between apps public_port
    let mut cache = HashMap::new();
    let mut implementation_cache = Vec::new();
    let mut apps_with_conflicts = Vec::new();
    // Process apps in such a way that installed apps are always processed first,
    // Then sort alphabetically (Also sort installed apps alphabetically)
    entries.sort_by(|a, b| {
        let a_installed = installed_apps.contains(&a.app);
        let b_installed = installed_apps.contains(&b.app);
        if a_installed && !b_installed {
            std::cmp::Ordering::Less
        } else if !a_installed && b_installed {
            std::cmp::Ordering::Greater
        } else {
            a.app.cmp(&b.app)
        }
    });
    for entry in entries {
        if apps_with_conflicts.contains(&entry.app) {
            continue;
        }
        if RESERVED_PORTS.contains(&entry.public_port) {
            if entry.priority == PortPriority::Required {
                apps_with_conflicts.push(entry.app.clone());
                // Remove any existing entries from this app
                cache.retain(|_, v: &mut PortMapEntry| v.app != entry.app);
            } else {
                // Move the entry to a new, free port
                let mut new_port = entry.public_port;
                while cache.contains_key(&new_port) || RESERVED_PORTS.contains(&new_port) {
                    new_port += 1;
                }
                let mut new_entry = entry.clone();
                new_entry.public_port = new_port;
                cache.insert(new_port, new_entry);
            }
        } else if cache.contains_key(&entry.public_port) {
            let other = cache.get(&entry.public_port).cloned().unwrap();
            if entry == other {
                continue;
            }
            if entry.implements.is_some()
                && other.implements.is_some()
                && entry.implements == other.implements
                && entry.priority == other.priority
                && entry.priority == PortPriority::Required
            {
                // If both entries implement the same app and are required, we can just ignore the other one
                implementation_cache.push(entry.clone());
                continue;
            }
            if entry.priority > other.priority {
                // Move the other entry to a new, free port
                let mut new_port = entry.public_port;
                while cache.contains_key(&new_port) || RESERVED_PORTS.contains(&new_port) {
                    new_port += 1;
                }
                let mut new_entry = other.clone();
                new_entry.public_port = new_port;
                cache.insert(new_port, new_entry);
                cache.insert(entry.public_port, entry);
            } else if entry.priority == PortPriority::Required {
                apps_with_conflicts.push(entry.app.clone());
                // Remove any existing entries from this app
                cache.retain(|_, v| v.app != entry.app);
            } else if entry.priority == other.priority {
                // To make sorting more deterministic, we'll use the app name as a tiebreaker
                if entry.app < other.app {
                    // Move the other entry to a new, free port
                    let mut new_port = entry.public_port;
                    while cache.contains_key(&new_port) || RESERVED_PORTS.contains(&new_port) {
                        new_port += 1;
                    }
                    let mut new_entry = other.clone();
                    new_entry.public_port = new_port;
                    cache.insert(new_port, new_entry);
                    cache.insert(entry.public_port, entry);
                } else {
                    // Move the entry to a new, free port
                    let mut new_port = entry.public_port;
                    while cache.contains_key(&new_port) || RESERVED_PORTS.contains(&new_port) {
                        new_port += 1;
                    }
                    let mut new_entry = entry.clone();
                    new_entry.public_port = new_port;
                    cache.insert(new_port, new_entry);
                }
            } else {
                // Move the entry to a new, free port
                let mut new_port = entry.public_port;
                while cache.contains_key(&new_port) || RESERVED_PORTS.contains(&new_port) {
                    new_port += 1;
                }
                let mut new_entry = entry.clone();
                new_entry.public_port = new_port;
                cache.insert(new_port, new_entry);
            }
        } else {
            cache.insert(entry.public_port, entry);
        }
    }
    let mut result: Vec<PortMapEntry> = cache.into_values().collect();
    result.append(&mut implementation_cache);
    // Sort by public port, then by app name in case of conflicts
    result.sort_by(|a, b| {
        if a.public_port == b.public_port {
            a.app.cmp(&b.app)
        } else {
            a.public_port.cmp(&b.public_port)
        }
    });
    (result, apps_with_conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod resolve_port_conflicts {
        use super::{resolve_port_conflicts, PortMapEntry, PortPriority};
        use pretty_assertions::assert_eq;
        #[test]
        fn basic() {
            let entries = vec![
                PortMapEntry {
                    app: "app1".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container1".to_owned(),
                    implements: None,
                    priority: PortPriority::Optional,
                },
                PortMapEntry {
                    app: "app2".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container2".to_owned(),
                    implements: None,
                    priority: PortPriority::Optional,
                },
                PortMapEntry {
                    app: "app3".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container3".to_owned(),
                    implements: None,
                    priority: PortPriority::Optional,
                },
            ];
            let (resolved, conflicts) = resolve_port_conflicts(entries, &[]);
            assert_eq!(
                resolved,
                vec![
                    PortMapEntry {
                        app: "app1".to_owned(),
                        internal_port: 80,
                        public_port: 81,
                        container: "container1".to_owned(),
                        implements: None,
                        priority: PortPriority::Optional,
                    },
                    PortMapEntry {
                        app: "app2".to_owned(),
                        internal_port: 80,
                        public_port: 82,
                        container: "container2".to_owned(),
                        implements: None,
                        priority: PortPriority::Optional,
                    },
                    PortMapEntry {
                        app: "app3".to_owned(),
                        internal_port: 80,
                        public_port: 83,
                        container: "container3".to_owned(),
                        implements: None,
                        priority: PortPriority::Optional,
                    },
                ]
            );
            assert!(conflicts.is_empty());
        }

        fn implementations() {
            let entries = vec![
                PortMapEntry {
                    app: "app1".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container1".to_owned(),
                    implements: Some("http".to_owned()),
                    priority: PortPriority::Optional,
                },
                PortMapEntry {
                    app: "app2".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container2".to_owned(),
                    implements: Some("http".to_owned()),
                    priority: PortPriority::Optional,
                },
                PortMapEntry {
                    app: "app3".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container3".to_owned(),
                    implements: Some("http".to_owned()),
                    priority: PortPriority::Optional,
                },
            ];
            let (resolved, conflicts) = resolve_port_conflicts(entries, &[]);
            assert_eq!(
                resolved,
                vec![
                    PortMapEntry {
                        app: "app1".to_owned(),
                        internal_port: 80,
                        public_port: 81,
                        container: "container1".to_owned(),
                        implements: Some("http".to_owned()),
                        priority: PortPriority::Optional,
                    },
                    PortMapEntry {
                        app: "app2".to_owned(),
                        internal_port: 80,
                        public_port: 81,
                        container: "container2".to_owned(),
                        implements: Some("http".to_owned()),
                        priority: PortPriority::Optional,
                    },
                    PortMapEntry {
                        app: "app3".to_owned(),
                        internal_port: 80,
                        public_port: 81,
                        container: "container3".to_owned(),
                        implements: Some("http".to_owned()),
                        priority: PortPriority::Optional,
                    },
                ]
            );
            assert!(conflicts.is_empty());
        }

        #[test]
        pub fn unresolvable_conflicts_between_apps() {
            let entries = vec![
                PortMapEntry {
                    app: "app1".to_owned(),
                    internal_port: 81,
                    public_port: 81,
                    container: "container1".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                },
                PortMapEntry {
                    app: "app2".to_owned(),
                    internal_port: 81,
                    public_port: 81,
                    container: "container2".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                },
            ];
            let (resolved, conflicts) = resolve_port_conflicts(entries, &[]);
            assert_eq!(
                resolved,
                vec![PortMapEntry {
                    app: "app1".to_owned(),
                    internal_port: 81,
                    public_port: 81,
                    container: "container1".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                }]
            );
            assert_eq!(conflicts, vec!["app2".to_owned()]);
        }

        #[test]
        pub fn unresolvable_conflicts_between_apps_and_installed() {
            let entries = vec![
                PortMapEntry {
                    app: "app1".to_owned(),
                    internal_port: 81,
                    public_port: 81,
                    container: "container1".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                },
                PortMapEntry {
                    app: "app2".to_owned(),
                    internal_port: 81,
                    public_port: 81,
                    container: "container2".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                },
            ];
            let (resolved, conflicts) = resolve_port_conflicts(entries, &["app2".to_owned()]);
            assert_eq!(
                resolved,
                vec![PortMapEntry {
                    app: "app2".to_owned(),
                    internal_port: 81,
                    public_port: 81,
                    container: "container2".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                }]
            );
            assert_eq!(conflicts, vec!["app1".to_owned()]);
        }

        #[test]
        fn unresolvable_conflicts_with_reserved() {
            let entries = vec![
                PortMapEntry {
                    app: "app1".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container1".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                },
                PortMapEntry {
                    app: "app2".to_owned(),
                    internal_port: 80,
                    public_port: 80,
                    container: "container2".to_owned(),
                    implements: None,
                    priority: PortPriority::Required,
                },
            ];
            let (resolved, conflicts) = resolve_port_conflicts(entries, &[]);
            assert!(resolved.is_empty());
            assert_eq!(conflicts, vec!["app1".to_owned(), "app2".to_owned()]);
        }
    }
}
