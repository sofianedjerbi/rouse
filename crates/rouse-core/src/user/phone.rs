use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Phone number validated in E.164 format (e.g., "+41791234567").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phone(String);

impl Phone {
    pub fn new(number: &str) -> Result<Self, DomainError> {
        if !Self::is_valid_e164(number) {
            return Err(DomainError::InvalidPhoneFormat);
        }
        Ok(Self(number.to_string()))
    }

    fn is_valid_e164(number: &str) -> bool {
        let bytes = number.as_bytes();
        if bytes.len() < 8 || bytes.len() > 16 {
            return false;
        }
        if bytes[0] != b'+' {
            return false;
        }
        bytes[1..].iter().all(|b| b.is_ascii_digit())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_valid_e164() {
        assert!(Phone::new("+41791234567").is_ok());
        assert!(Phone::new("+12025551234").is_ok());
        assert!(Phone::new("+447911123456").is_ok());
    }

    #[test]
    fn phone_invalid_rejects() {
        assert_eq!(
            Phone::new("041791234567"),
            Err(DomainError::InvalidPhoneFormat)
        );
        assert_eq!(Phone::new("+123"), Err(DomainError::InvalidPhoneFormat));
        assert_eq!(Phone::new(""), Err(DomainError::InvalidPhoneFormat));
        assert_eq!(
            Phone::new("+41-791-234-567"),
            Err(DomainError::InvalidPhoneFormat)
        );
    }
}
