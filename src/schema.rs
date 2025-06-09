// @generated automatically by Diesel CLI.

diesel::table! {
    messages (id) {
        id -> Int4,
        #[max_length = 255]
        thread_id -> Varchar,
        content -> Nullable<Text>,
        role -> Varchar,
        active_model -> Varchar,
        active_lab -> Varchar,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
        user_id -> Nullable<Int4>,
    }
}

diesel::table! {
    threads (id) {
        #[max_length = 255]
        id -> Varchar,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
        user_id -> Nullable<Int4>,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        external_id -> Varchar,
        provider -> Varchar,
        email -> Nullable<Varchar>,
        username -> Nullable<Varchar>,
        display_name -> Nullable<Varchar>,
        avatar_url -> Nullable<Varchar>,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}

diesel::joinable!(messages -> threads (thread_id));
diesel::joinable!(messages -> users (user_id));
diesel::joinable!(threads -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    messages,
    threads,
    users,
);
