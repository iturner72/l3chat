use cfg_if::cfg_if;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThreadView {
    pub id: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageView {
    pub id: i32,
    pub thread_id: String,
    pub content: Option<String>,
    pub role: String,
    pub active_model: String,
    pub active_lab: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewMessageView {
    pub thread_id: String,
    pub content: Option<String>,
    pub role: String,
    pub active_model: String,
    pub active_lab: String,
}

cfg_if! { if #[cfg(feature = "ssr")] {
    use crate::schema::*;
    use chrono::NaiveDateTime;
    use diesel::prelude::*;

    #[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable)]
    #[diesel(table_name = threads)]
    pub struct Thread {
        #[diesel(column_name = id)]
        pub id: String,
        pub created_at: Option<NaiveDateTime>,
        pub updated_at: Option<NaiveDateTime>,
    }

    impl From<Thread> for ThreadView {
        fn from(thread: Thread) -> Self {
            ThreadView {
                id: thread.id,
                created_at: thread.created_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                updated_at: thread.updated_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            }
        }
    }

    // used for querying messages directly from the database
    #[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, Associations, Default)]
    #[diesel(belongs_to(Thread, foreign_key = thread_id))]
    #[diesel(table_name = messages)]
    pub struct Message {
        pub id: i32,
        #[diesel(column_name = thread_id)]
        pub thread_id: String,
        pub content: Option<String>,
        pub role: String,
        pub active_model: String,
        pub active_lab: String,
        pub created_at: Option<NaiveDateTime>,
        pub updated_at: Option<NaiveDateTime>,
    }

    impl From<Message> for MessageView {
        fn from(message: Message) -> Self {
            MessageView {
                id: message.id,
                thread_id: message.thread_id,
                content: message.content,
                role: message.role,
                active_model: message.active_model,
                active_lab: message.active_lab,
                created_at: message.created_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                updated_at: message.updated_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            }
        }
    }

    // message data from the client ("new type" or "insert type" pattern)
    #[derive(Debug, Insertable, Deserialize)]
    #[diesel(table_name = messages)]
    pub struct NewMessage {
        pub thread_id: String,
        pub content: Option<String>,
        pub role: String,
        pub active_model: String,
        pub active_lab: String,
    }

    impl From<NewMessageView> for NewMessage {
        fn from (view: NewMessageView) -> Self {
            NewMessage {
                thread_id: view.thread_id,
                content: view.content,
                role: view.role,
                active_model: view.active_model,
                active_lab: view.active_lab,
            }
        }
    }
}}
