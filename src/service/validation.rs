//! Input validation for user-facing fields (nickname, password, etc.)
//!
//! These functions return a [`crate::service::Error`] on failure, so they can
//! be used directly in API handlers with the `?` operator.

/// Validate an Aphanite nickname.
///
/// Rules:
/// - Length: maximum of 20 characters
///
/// Note: Nicknames are NOT used as the unique identifier of an account so special characters ARE allowed.
pub fn validate_nickname(name: &str) -> Result<(), super::Error> {
    let len = name.chars().count();
    if !(1..=20).contains(&len) {
        return Err(super::Error::error(
            418,
            format!(
                "Nickname must not exceed 20 characters, got {} character{}",
                len,
                if len == 1 { "" } else { "s" },
            ),
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
