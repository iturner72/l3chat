use cfg_if::cfg_if;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThreadView {
    pub id: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub user_id: Option<i32>,
    pub parent_thread_id: Option<String>,
    pub branch_point_message_id: Option<i32>,
    pub branch_name: Option<String>,
    pub title: Option<String>,
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
    pub user_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewMessageView {
    pub thread_id: String,
    pub content: Option<String>,
    pub role: String,
    pub active_model: String,
    pub active_lab: String,
    pub user_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchInfo {
    pub thread_id: String,
    pub branch_name: Option<String>,
    pub model: String,
    pub lab: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PendingMessage {
    pub id: String,
    pub thread_id: String,
    pub content: String,
    pub role: String,
    pub active_model: String,
    pub active_lab: String,
    pub is_streaming: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum DisplayMessage {
    Persisted(MessageView),
    Pending(PendingMessage),
}

impl DisplayMessage {
    pub fn id(&self) -> String {
        match self {
            DisplayMessage::Persisted(msg) => msg.id.to_string(),
            DisplayMessage::Pending(msg) => msg.id.clone(),
        }
    }

    pub fn thread_id(&self) -> &str {
        match self {
            DisplayMessage::Persisted(msg) => &msg.thread_id,
            DisplayMessage::Pending(msg) => &msg.thread_id,
        }
    }

    pub fn content(&self) -> &str {
        match self {
            DisplayMessage::Persisted(msg) => msg.content.as_deref().unwrap_or(""),
            DisplayMessage::Pending(msg) => &msg.content,
        }
    }

    pub fn role(&self) -> &str {
        match self {
            DisplayMessage::Persisted(msg) => &msg.role,
            DisplayMessage::Pending(msg) => &msg.role,
        }
    }

    pub fn active_model(&self) -> &str {
        match self {
            DisplayMessage::Persisted(msg) => &msg.active_model,
            DisplayMessage::Pending(msg) => &msg.active_model,
        }
    }

    pub fn active_lab(&self) -> &str {
        match self {
            DisplayMessage::Persisted(msg) => &msg.active_lab,
            DisplayMessage::Pending(msg) => &msg.active_lab,
        }
    }

    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        match self {
            DisplayMessage::Persisted(msg) => msg.created_at,
            DisplayMessage::Pending(msg) => Some(msg.created_at),
        }
    }

    pub fn is_streaming(&self) -> bool {
        match self {
            DisplayMessage::Persisted(_) => false,
            DisplayMessage::Pending(msg) => msg.is_streaming,
        }
    }

    pub fn is_user(&self) -> bool {
        self.role() == "user"
    }

    pub fn db_id(&self) -> Option<i32> {
        match self {
            DisplayMessage::Persisted(msg) => Some(msg.id),
            DisplayMessage::Pending(_) => None,
        }
    }
}

cfg_if! { if #[cfg(feature = "ssr")] {
    use crate::schema::*;
    use chrono::NaiveDateTime;
    use diesel::prelude::*;

    #[derive(Debug, Serialize, Deserialize, Queryable, QueryableByName,  Identifiable, Insertable)]
    #[diesel(table_name = threads)]
    pub struct Thread {
        #[diesel(column_name = id)]
        pub id: String,
        pub created_at: Option<NaiveDateTime>,
        pub updated_at: Option<NaiveDateTime>,
        pub user_id: Option<i32>,
        pub parent_thread_id: Option<String>,
        pub branch_point_message_id: Option<i32>,
        pub branch_name: Option<String>,
        pub title: Option<String>,
    }

    impl From<Thread> for ThreadView {
        fn from(thread: Thread) -> Self {
            ThreadView {
                id: thread.id,
                created_at: thread.created_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                updated_at: thread.updated_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                user_id: thread.user_id,
                parent_thread_id: thread.parent_thread_id,
                branch_point_message_id: thread.branch_point_message_id,
                branch_name: thread.branch_name,
                title: thread.title,
            }
        }
    }

    // used for querying messages directly from the database
    #[derive(Debug, Serialize, Deserialize, Queryable, QueryableByName, Identifiable, Insertable, Associations, Default)]
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
        pub user_id: Option<i32>,
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
                user_id: message.user_id,
            }
        }
    }

    // message data from the client ("new type" or "insert type" pattern)
    #[derive(Debug, Insertable, Deserialize, QueryableByName)]
    #[diesel(table_name = messages)]
    pub struct NewMessage {
        pub thread_id: String,
        pub content: Option<String>,
        pub role: String,
        pub active_model: String,
        pub active_lab: String,
        pub user_id: Option<i32>,
    }

    impl From<NewMessageView> for NewMessage {
        fn from (view: NewMessageView) -> Self {
            NewMessage {
                thread_id: view.thread_id,
                content: view.content,
                role: view.role,
                active_model: view.active_model,
                active_lab: view.active_lab,
                user_id: view.user_id,
            }
        }
    }
}}
