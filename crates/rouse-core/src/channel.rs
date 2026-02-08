use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    Slack,
    Discord,
    Telegram,
    WhatsApp,
    Sms,
    Phone,
    Email,
    Webhook,
}
