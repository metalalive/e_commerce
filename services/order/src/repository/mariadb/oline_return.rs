use std::sync::Arc;

use crate::datastore::AppMariaDbStore;

pub(crate) struct OrderReturnMariaDbRepo {
    _db : Arc<AppMariaDbStore>,
}
