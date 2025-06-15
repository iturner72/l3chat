// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    chunk_embeddings (chunk_id) {
        chunk_id -> Uuid,
        embedding -> Nullable<Vector>,
        #[max_length = 100]
        embedding_model -> Nullable<Varchar>,
        created_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    document_chunks (id) {
        id -> Uuid,
        document_id -> Uuid,
        chunk_text -> Text,
        chunk_index -> Int4,
        start_char -> Nullable<Int4>,
        end_char -> Nullable<Int4>,
        metadata -> Nullable<Jsonb>,
        created_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

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
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    project_documents (id) {
        id -> Uuid,
        project_id -> Uuid,
        #[max_length = 255]
        filename -> Varchar,
        content -> Text,
        #[max_length = 100]
        content_type -> Nullable<Varchar>,
        file_size -> Nullable<Int4>,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    projects (id) {
        id -> Uuid,
        user_id -> Int4,
        #[max_length = 255]
        name -> Varchar,
        description -> Nullable<Text>,
        instructions -> Nullable<Text>,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    threads (id) {
        #[max_length = 255]
        id -> Varchar,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
        user_id -> Nullable<Int4>,
        #[max_length = 255]
        parent_thread_id -> Nullable<Varchar>,
        branch_point_message_id -> Nullable<Int4>,
        #[max_length = 255]
        branch_name -> Nullable<Varchar>,
        #[max_length = 255]
        title -> Nullable<Varchar>,
        project_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

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

diesel::joinable!(chunk_embeddings -> document_chunks (chunk_id));
diesel::joinable!(document_chunks -> project_documents (document_id));
diesel::joinable!(messages -> users (user_id));
diesel::joinable!(project_documents -> projects (project_id));
diesel::joinable!(projects -> users (user_id));
diesel::joinable!(threads -> projects (project_id));
diesel::joinable!(threads -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    chunk_embeddings,
    document_chunks,
    messages,
    project_documents,
    projects,
    threads,
    users,
);
