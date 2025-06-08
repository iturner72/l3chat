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
    }
}

diesel::table! {
    threads (id) {
        #[max_length = 255]
        id -> Varchar,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}

diesel::joinable!(messages -> threads (thread_id));

diesel::allow_tables_to_appear_in_same_query!(messages, threads,);
