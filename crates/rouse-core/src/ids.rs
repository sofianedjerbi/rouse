use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DomainError;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn parse(s: &str) -> Result<Self, DomainError> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|_| DomainError::InvalidId(stringify!($name).into()))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

define_id!(AlertId);
define_id!(UserId);
define_id!(ScheduleId);
define_id!(PolicyId);
define_id!(TeamId);
define_id!(GroupId);
define_id!(OverrideId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_uuid_succeeds() {
        let id = AlertId::new();
        let parsed = AlertId::parse(&id.to_string()).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn parse_invalid_uuid_fails() {
        let result = AlertId::parse("not-a-uuid");
        assert_eq!(result, Err(DomainError::InvalidId("AlertId".into())));
    }

    #[test]
    fn different_id_types_are_distinct() {
        // This is a compile-time guarantee — just verify they exist
        let _alert = AlertId::new();
        let _user = UserId::new();
        let _schedule = ScheduleId::new();
        let _policy = PolicyId::new();
        let _team = TeamId::new();
        let _group = GroupId::new();
        let _override_id = OverrideId::new();
    }
}
