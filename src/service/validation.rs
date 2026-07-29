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

/// Validate a Minecraft player profile name.
///
/// Rules:
/// - Length: 3–16 characters (inclusive)
/// - Characters: only `a–z`, `A–Z`, `0–9`, and `_`
pub fn validate_profile_name(name: &str) -> Result<(), super::Error> {
    let len = name.len();
    if !(3..=16).contains(&len) {
        return Err(super::Error::error(
            400,
            format!("Profile name must be between 3 and 16 characters, got {len}"),
        ));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(super::Error::error(
            400,
            "Profile name must only contain letters, numbers, and underscores",
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
