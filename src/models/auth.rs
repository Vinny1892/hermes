//! Authentication and authorisation types shared between client and server.

use serde::{Deserialize, Serialize};

/// Access level granted to a user account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    Admin,
    User,
    Guest,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "ADMIN",
            Role::User => "USER",
            Role::Guest => "GUEST",
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "ADMIN" => Ok(Role::Admin),
            "USER" => Ok(Role::User),
            "GUEST" => Ok(Role::Guest),
            other => Err(format!("unknown role: {other}")),
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returned by the `login_user` server function on success.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Opaque session token — store client-side and pass to authenticated calls.
    pub token: String,
    pub email: String,
    pub role: Role,
}

/// Lightweight user info returned by `get_session_user`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub role: Role,
}
