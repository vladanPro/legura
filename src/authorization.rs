use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Admin,
    Editor,
    Subscriber,
}

impl Role {
    fn from_database(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(Self::Admin),
            "editor" => Some(Self::Editor),
            "subscriber" => Some(Self::Subscriber),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Capability {
    Administer,
    ManageUsers,
    ManageContent,
    ManageOwnProfile,
}

impl Capability {
    fn from_name(value: &str) -> Option<Self> {
        match value {
            "administer" => Some(Self::Administer),
            "manage_users" => Some(Self::ManageUsers),
            "manage_content" => Some(Self::ManageContent),
            "manage_own_profile" => Some(Self::ManageOwnProfile),
            _ => None,
        }
    }
}

fn allows(role: Option<Role>, capability: Option<Capability>) -> bool {
    matches!(
        (role, capability),
        (Some(Role::Admin), Some(_))
            | (Some(Role::Editor), Some(Capability::ManageContent))
            | (Some(_), Some(Capability::ManageOwnProfile))
    )
}

pub(crate) fn allows_own_profile(role: &str) -> bool {
    allows(
        Role::from_database(role),
        Some(Capability::ManageOwnProfile),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessCheck {
    role: String,
    capability: String,
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.auth.landing", |role: String| {
        Ok::<_, ()>(landing_route(Role::from_database(&role)))
    })?;
    registry.register("legura.auth.allows", |input: AccessCheck| {
        Ok::<_, ()>(allows(
            Role::from_database(&input.role),
            Capability::from_name(&input.capability),
        ))
    })
}

fn landing_route(role: Option<Role>) -> String {
    match role {
        Some(Role::Admin) => "/admin",
        Some(Role::Editor) => "/admin/posts",
        Some(Role::Subscriber) => "/admin/profile",
        None => "",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_targets_are_fixed_by_role() {
        assert_eq!(landing_route(Some(Role::Admin)), "/admin");
        assert_eq!(landing_route(Some(Role::Editor)), "/admin/posts");
        assert_eq!(landing_route(Some(Role::Subscriber)), "/admin/profile");
        assert_eq!(landing_route(Role::from_database("unknown")), "");
    }

    #[test]
    fn role_policy_denies_unknown_roles_and_capabilities() {
        for (role, expected) in [
            ("admin", [true, true, true, true, false]),
            ("editor", [false, false, true, true, false]),
            ("subscriber", [false, false, false, true, false]),
            ("owner", [false; 5]),
            ("", [false; 5]),
        ] {
            for (capability, expected) in [
                "administer",
                "manage_users",
                "manage_content",
                "manage_own_profile",
                "unknown",
            ]
            .into_iter()
            .zip(expected)
            {
                assert_eq!(
                    allows(Role::from_database(role), Capability::from_name(capability)),
                    expected,
                    "{role}: {capability}"
                );
            }
        }
    }
}
