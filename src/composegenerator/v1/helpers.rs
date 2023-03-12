use crate::composegenerator::types::Permission;

/// Find the best permission that matches, or None if none matches
/// app_name is the apps these permissions are exposed by, not the app using them
pub fn find_permission_that_matches<'a, P>(
    app_name: &str,
    perms: &'a [Permission],
    current_permissions: &[String],
    check: P,
) -> Option<&'a Permission>
where
    P: FnMut(&&Permission) -> bool,
{
    let mut perms_that_expose_this_var = perms.iter().filter(check).collect::<Vec<_>>();
    if perms_that_expose_this_var.is_empty() {
        None
    } else if perms_that_expose_this_var.len() == 1 {
        return Some(perms_that_expose_this_var[0]);
    } else {
        for perm in perms_that_expose_this_var.iter() {
            if current_permissions.contains(&format!("{}/{}", app_name, perm.id)) {
                return Some(perm);
            }
        }
        perms_that_expose_this_var.sort_by(|a, b| {
            a.includes
                .len()
                .cmp(&b.includes.len())
                .then(a.id.cmp(&b.id))
        });
        return Some(perms_that_expose_this_var[0]);
    }
}
