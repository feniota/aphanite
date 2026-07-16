//! Input validation for user-facing fields (nickname, password, etc.)
//!
//! These functions return a [`crate::service::Error`] on failure, so they can
//! be used directly in API handlers with the `?` operator.

/// Validate a nickname/username.
///
/// Rules:
/// - Length: 3–16 characters (inclusive)
/// - Allowed characters: `a-z`, `A-Z`, `0-9`, `_`, `-`
pub fn validate_nickname(name: &str) -> Result<(), super::Error> {
    let len = name.chars().count();
    if !(3..=16).contains(&len) {
        return Err(super::Error::error(
            422,
            format!(
                "Nickname must be 3-16 characters, got {} character{}",
                len,
                if len == 1 { "" } else { "s" },
            ),
        ));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(super::Error::error(
            422,
            "Nickname may only contain letters, digits, underscores, and hyphens",
        ));
    }

    Ok(())
}

/// Validate a password.
///
/// Rules:
/// - Length: 8–128 characters (inclusive)
pub fn validate_password(password: &str) -> Result<(), super::Error> {
    let len = password.len();
    if len < 8 {
        return Err(super::Error::error(
            422,
            format!("Password must be at least 8 characters, got {len}",),
        ));
    }
    if len > 128 {
        return Err(super::Error::error(
            422,
            format!("Password must be at most 128 characters, got {len}",),
        ));
    }

    Ok(())
}
