//! Authentication and authorization.
//!
//! A minimal, dependency-light bearer-token scheme: tokens are configured
//! via the `STELLARTRACE_API_TOKENS` environment variable (see README),
//! each mapped to an investigator id and role. This is intentionally
//! simple to keep the reference implementation self-contained; swapping
//! in OAuth2/OIDC or a real identity provider means replacing
//! `AuthRegistry::authenticate` only — every handler already goes through
//! the same `AuthUser` extractor and role check.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use std::collections::HashMap;
use stellartrace_common::Role;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub investigator_id: String,
    pub role: Role,
}

pub struct AuthRegistry {
    tokens: HashMap<String, AuthUser>,
}

impl AuthRegistry {
    /// Parses `STELLARTRACE_API_TOKENS` of the form
    /// `token:role:investigator_id,token2:role2:investigator_id2`.
    /// Falls back to a single dev token if unset, logged loudly so it is
    /// never mistaken for a production configuration.
    pub fn from_env() -> Self {
        let mut tokens = HashMap::new();
        match std::env::var("STELLARTRACE_API_TOKENS") {
            Ok(raw) => {
                for entry in raw.split(',').filter(|s| !s.trim().is_empty()) {
                    let parts: Vec<&str> = entry.trim().splitn(3, ':').collect();
                    if parts.len() != 3 {
                        tracing::warn!(entry, "skipping malformed STELLARTRACE_API_TOKENS entry");
                        continue;
                    }
                    let role = match parts[1] {
                        "admin" => Role::Admin,
                        "investigator" => Role::Investigator,
                        "viewer" => Role::Viewer,
                        other => {
                            tracing::warn!(
                                role = other,
                                "unknown role in STELLARTRACE_API_TOKENS entry, skipping"
                            );
                            continue;
                        }
                    };
                    tokens.insert(
                        parts[0].to_string(),
                        AuthUser {
                            investigator_id: parts[2].to_string(),
                            role,
                        },
                    );
                }
            }
            Err(_) => {
                tracing::warn!(
                    "STELLARTRACE_API_TOKENS not set; using an insecure default dev token. \
                     Do not use this configuration in production."
                );
                tokens.insert(
                    "dev-investigator-token".to_string(),
                    AuthUser {
                        investigator_id: "dev-investigator".to_string(),
                        role: Role::Investigator,
                    },
                );
            }
        }
        Self { tokens }
    }

    pub fn authenticate(&self, token: &str) -> Option<AuthUser> {
        self.tokens.get(token).cloned()
    }
}

pub enum AuthError {
    Missing,
    Invalid,
}

impl axum::response::IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let msg = match self {
            AuthError::Missing => "missing bearer token",
            AuthError::Invalid => "invalid or expired token",
        };
        (StatusCode::UNAUTHORIZED, msg).into_response()
    }
}

#[axum::async_trait]
impl FromRequestParts<std::sync::Arc<crate::state::AppState>> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &std::sync::Arc<crate::state::AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::Missing)?;

        let token = header.strip_prefix("Bearer ").ok_or(AuthError::Missing)?;
        state.auth.authenticate(token).ok_or(AuthError::Invalid)
    }
}
