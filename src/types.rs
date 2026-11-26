//! The general data models and types used across the whole Aphanite service

/// Permission types of a user
///
/// This is stored in u32 bitflags. See the provided methods for how to interact with this type.
#[derive(Clone, Copy, strum::EnumIter, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// This user is allowed to Manage this Aphanite instance
    ///
    /// Bit: `0b1`
    Management,
}

impl Permission {
    /// Return a number with the corresponding bit set as 1
    fn as_u32(&self) -> u32 {
        match self {
            Self::Management => 0b1,
        }
    }

    /// Parse a set of flags from a u32 number
    pub fn parse_flags(flags: u32) -> Vec<Self> {
        use strum::IntoEnumIterator;

        let mut result = Vec::new();
        for permission in Self::iter() {
            if flags & permission.as_u32() != 0 {
                result.push(permission);
            }
        }
        result
    }

    /// Construct a number from a set of [`Permission`](Self)s
    pub fn construct_number(flags: &[Self]) -> u32 {
        let mut result = 0;
        for perm in flags.iter() {
            result |= perm.as_u32();
        }
        result
    }
}

pub trait ToPermission {
    /// If the number has the corresponding bit set as 1, this returns true; otherwise false.
    fn contains(&self, permission: Permission) -> bool;
}
impl ToPermission for u32 {
    fn contains(&self, permission: Permission) -> bool {
        self & permission.as_u32() != 0
    }
}

impl ToPermission for crate::data::Unsigned32 {
    fn contains(&self, permission: Permission) -> bool {
        self.0.contains(permission)
    }
}
