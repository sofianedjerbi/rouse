pub mod phone;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::ids::{TeamId, UserId};

pub use phone::Phone;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Admin,
    User,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    id: UserId,
    username: String,
    email: String,
    slack_id: Option<String>,
    discord_id: Option<String>,
    telegram_id: Option<String>,
    whatsapp_id: Option<String>,
    phone: Option<Phone>,
    role: Role,
}

impl User {
    pub fn new(username: String, email: String, role: Role) -> Self {
        Self {
            id: UserId::new(),
            username,
            email,
            slack_id: None,
            discord_id: None,
            telegram_id: None,
            whatsapp_id: None,
            phone: None,
            role,
        }
    }

    pub fn can_be_on_call(&self) -> bool {
        self.phone.is_some()
            || self.slack_id.is_some()
            || self.discord_id.is_some()
            || self.telegram_id.is_some()
            || self.whatsapp_id.is_some()
    }

    pub fn set_phone(&mut self, phone: Phone) {
        self.phone = Some(phone);
    }

    pub fn set_slack_id(&mut self, id: String) {
        self.slack_id = Some(id);
    }

    pub fn set_discord_id(&mut self, id: String) {
        self.discord_id = Some(id);
    }

    pub fn set_telegram_id(&mut self, id: String) {
        self.telegram_id = Some(id);
    }

    pub fn set_whatsapp_id(&mut self, id: String) {
        self.whatsapp_id = Some(id);
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn phone(&self) -> Option<&Phone> {
        self.phone.as_ref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    id: TeamId,
    name: String,
    members: Vec<UserId>,
}

impl Team {
    pub fn new(name: String, members: Vec<UserId>) -> Result<Self, DomainError> {
        if members.is_empty() {
            return Err(DomainError::TeamRequiresMember);
        }
        Ok(Self {
            id: TeamId::new(),
            name,
            members,
        })
    }

    pub fn id(&self) -> &TeamId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn members(&self) -> &[UserId] {
        &self.members
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_can_be_on_call_with_phone() {
        let mut user = User::new("alice".into(), "alice@test.com".into(), Role::User);
        user.set_phone(Phone::new("+41791234567").unwrap());
        assert!(user.can_be_on_call());
    }

    #[test]
    fn user_can_be_on_call_with_slack() {
        let mut user = User::new("alice".into(), "alice@test.com".into(), Role::User);
        user.set_slack_id("U12345".into());
        assert!(user.can_be_on_call());
    }

    #[test]
    fn user_can_be_on_call_with_whatsapp() {
        let mut user = User::new("alice".into(), "alice@test.com".into(), Role::User);
        user.set_whatsapp_id("+41791234567".into());
        assert!(user.can_be_on_call());
    }

    #[test]
    fn user_cannot_be_on_call_no_contact() {
        let user = User::new("alice".into(), "alice@test.com".into(), Role::User);
        assert!(!user.can_be_on_call());
    }

    #[test]
    fn team_requires_member() {
        let result = Team::new("empty".into(), vec![]);
        assert!(matches!(result, Err(DomainError::TeamRequiresMember)));
    }

    #[test]
    fn team_with_members_succeeds() {
        let team = Team::new("backend".into(), vec![UserId::new()]);
        assert!(team.is_ok());
    }
}
