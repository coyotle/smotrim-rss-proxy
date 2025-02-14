use lazy_static::lazy_static;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct RssCache {
    pub body: String,
    pub cached_at: i64, // timestamp последнего обновления в UTC
}

lazy_static! {
    pub static ref FEEDS_CACHE: Mutex<HashMap<String, RssCache>> = Mutex::new(HashMap::new());
}
