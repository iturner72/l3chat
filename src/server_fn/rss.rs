use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RssProgressUpdate {
    pub company: String,
    pub status: String,
    pub new_posts: i32,
    pub skipped_posts: i32,
    pub current_post: Option<String>,
}

#[cfg(feature = "ssr")]
mod ssr {
    use super::RssProgressUpdate;
    use axum::response::sse::Event;
    use std::convert::Infallible;

    impl RssProgressUpdate {
        pub fn into_event(self) -> Result<Event, Infallible> {
            Ok(Event::default().data(serde_json::to_string(&self).unwrap_or_default()))
        }
    }
}
