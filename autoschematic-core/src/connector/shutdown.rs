use tokio::sync::RwLock;

lazy_static::lazy_static! {
    pub static ref ABORT_HANDLE:
        RwLock<Option<tokio::task::AbortHandle>>
            = RwLock::new(None);
}
