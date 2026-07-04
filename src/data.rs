//! [`DatabaseAccessor`](crate::data::DatabaseAccessor) and everything (except storage) related to database

use std::sync::Arc;
use toasty::Db;

#[derive(Clone)]
pub struct DatabaseAccessor {
    db: Db,
}

impl DatabaseAccessor {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}
