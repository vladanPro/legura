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
}

impl Capability {
    fn from_name(value: &str) -> Option<Self> {
        match value {
            "administer" => Some(Self::Administer),
            "manage_users" => Some(Self::ManageUsers),
            "manage_content" => Some(Self::ManageContent),
            _ => None,
        }
    }
}

fn allows(role: Option<Role>, capability: Option<Capability>) -> bool {
    matches!(
        (role, capability),
        (Some(Role::Admin), Some(_)) | (Some(Role::Editor), Some(Capability::ManageContent))
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessCheck {
    role: String,
    capability: String,
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.auth.allows", |input: AccessCheck| {
        Ok::<_, ()>(allows(
            Role::from_database(&input.role),
            Capability::from_name(&input.capability),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_policy_denies_unknown_roles_and_capabilities() {
        for (role, expected) in [
            ("admin", [true, true, true, false]),
            ("editor", [false, false, true, false]),
            ("subscriber", [false; 4]),
            ("owner", [false; 4]),
            ("", [false; 4]),
        ] {
            for (capability, expected) in
                ["administer", "manage_users", "manage_content", "unknown"]
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
