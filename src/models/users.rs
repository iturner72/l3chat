use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserView {
    pub id: i32,
    pub external_id: String,
    pub provider: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateUserView {
    pub external_id: String,
    pub provider: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

cfg_if! { if #[cfg(feature = "ssr")] {
    use crate::schema::*;
    use chrono::NaiveDateTime;
    use diesel::prelude::*;

    #[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable)]
    #[diesel(table_name = users)]
    pub struct User {
        pub id: i32,
        pub external_id: String,
        pub provider: String,
        pub email: Option<String>,
        pub username: Option<String>,
        pub display_name: Option<String>,
        pub avatar_url: Option<String>,
        pub created_at: Option<NaiveDateTime>,
        pub updated_at: Option<NaiveDateTime>,
    }

    #[derive(Debug, Insertable)]
    #[diesel(table_name = users)]
    pub struct NewUser {
        pub external_id: String,
        pub provider: String,
        pub email: Option<String>,
        pub username: Option<String>,
        pub display_name: Option<String>,
        pub avatar_url: Option<String>,
    }

    impl From<User> for UserView {
        fn from(user: User) -> Self {
            UserView {
                id: user.id,
                external_id: user.external_id,
                provider: user.provider,
                email: user.email,
                username: user.username,
                display_name: user.display_name,
                avatar_url: user.avatar_url,
            }
        }
    }

    impl From<CreateUserView> for NewUser {
        fn from(view: CreateUserView) -> Self {
            NewUser {
                external_id: view.external_id,
                provider: view.provider,
                email: view.email,
                username: view.username,
                display_name: view.display_name,
                avatar_url: view.avatar_url,
            }
        }
    }
}}
