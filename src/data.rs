//! [`DatabaseAccessor`](crate::data::DatabaseAccessor) and everything (except storage) related to database

use std::sync::Arc;
use toasty::Db;

#[derive(Clone)]
pub struct DatabaseAccessor {
    db: Arc<Db>,
}

impl DatabaseAccessor {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    #[inline]
    fn as_db(&self) -> Db {
        Db::clone(self.db.as_ref())
    }
}
