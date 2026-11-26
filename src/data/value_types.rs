//! Unsigned Rust values stored in signed SQL columns for PostgreSQL compatibility.

use sea_orm::sea_query::{ArrayType, ColumnType, Nullable, ValueType, ValueTypeErr};
use sea_orm::{ColIdx, DbErr, QueryResult, TryGetError, TryGetable, Value};

macro_rules! unsigned_value {
    ($name:ident, $unsigned:ty, $signed:ty) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $unsigned);

        impl From<$unsigned> for $name {
            fn from(value: $unsigned) -> Self {
                Self(value)
            }
        }

        impl From<$name> for $unsigned {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl From<$name> for Value {
            fn from(value: $name) -> Self {
                <$signed>::from(value.0).into()
            }
        }

        impl TryGetable for $name {
            fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
                let value = <$signed as TryGetable>::try_get_by(res, idx)?;
                <$unsigned as TryFrom<$signed>>::try_from(value)
                    .map(Self)
                    .map_err(|error| {
                        TryGetError::DbErr(DbErr::Type(format!(
                            "Invalid {}: {error}",
                            stringify!($name)
                        )))
                    })
            }
        }

        impl ValueType for $name {
            fn try_from(value: Value) -> Result<Self, ValueTypeErr> {
                let value = <$signed as ValueType>::try_from(value)?;
                <$unsigned as TryFrom<$signed>>::try_from(value)
                    .map(Self)
                    .map_err(|_| ValueTypeErr)
            }

            fn type_name() -> String {
                stringify!($name).to_owned()
            }

            fn array_type() -> ArrayType {
                <$signed as ValueType>::array_type()
            }

            fn column_type() -> ColumnType {
                <$signed as ValueType>::column_type()
            }
        }

        impl Nullable for $name {
            fn null() -> Value {
                <$signed as Nullable>::null()
            }
        }

        impl sea_orm::IntoActiveValue<$name> for $name {
            fn into_active_value(self) -> sea_orm::ActiveValue<$name> {
                sea_orm::Set(self)
            }
        }
    };
}

// PostgreSQL has no unsigned integer columns; widening preserves the full Rust range.
unsigned_value!(Unsigned32, u32, i64);
unsigned_value!(Unsigned16, u16, i32);
