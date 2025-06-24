use cfg_if::cfg_if;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectView {
    pub id: Uuid,
    pub user_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectDocumentView {
    pub id: Uuid,
    pub project_id: Uuid,
    pub filename: String,
    pub content: String,
    pub content_type: Option<String>,
    pub file_size: Option<i32>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentChunkView {
    pub id: Uuid,
    pub document_id: Uuid,
    pub chunk_text: String,
    pub chunk_index: i32,
    pub start_char: Option<i32>,
    pub end_char: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkEmbeddingView {
    pub chunk_id: Uuid,
    pub embedding: Option<Vec<f32>>,
    pub embedding_model: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectSearchResult {
    pub chunk_id: Uuid,
    pub chunk_text: String,
    pub similarity: f32,
    pub document_id: Uuid,
    pub filename: String,
    pub chunk_index: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewProjectView {
    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewDocumentView {
    pub project_id: Uuid,
    pub filename: String,
    pub content: String,
    pub content_type: Option<String>,
}

cfg_if! { if #[cfg(feature = "ssr")] {
    use crate::schema::*;
    use crate::models::users::User;
    use chrono::NaiveDateTime;
    use diesel::prelude::*;
    use pgvector::Vector;

    #[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, Associations)]
    #[diesel(belongs_to(User, foreign_key = user_id))]
    #[diesel(table_name = projects)]
    pub struct Project {
        pub id: Uuid,
        pub user_id: i32,
        pub name: String,
        pub description: Option<String>,
        pub instructions: Option<String>,
        pub created_at: Option<NaiveDateTime>,
        pub updated_at: Option<NaiveDateTime>,
    }

    #[derive(Debug, Insertable, Associations)]
    #[diesel(belongs_to(User, foreign_key = user_id))]
    #[diesel(table_name = projects)]
    pub struct NewProject {
        pub user_id: i32,
        pub name: String,
        pub description: Option<String>,
        pub instructions: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, Associations)]
    #[diesel(belongs_to(Project, foreign_key = project_id))]
    #[diesel(table_name = project_documents)]
    pub struct ProjectDocument {
        pub id: Uuid,
        pub project_id: Uuid,
        pub filename: String,
        pub content: String,
        pub content_type: Option<String>,
        pub file_size: Option<i32>,
        pub created_at: Option<NaiveDateTime>,
        pub updated_at: Option<NaiveDateTime>,
    }

    #[derive(Debug, Insertable, Associations)]
    #[diesel(belongs_to(Project, foreign_key = project_id))]
    #[diesel(table_name = project_documents)]
    pub struct NewProjectDocument {
        pub project_id: Uuid,
        pub filename: String,
        pub content: String,
        pub content_type: Option<String>,
        pub file_size: Option<i32>,
    }

    #[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, Associations)]
    #[diesel(belongs_to(ProjectDocument, foreign_key = document_id))]
    #[diesel(table_name = document_chunks)]
    pub struct DocumentChunk {
        pub id: Uuid,
        pub document_id: Uuid,
        pub chunk_text: String,
        pub chunk_index: i32,
        pub start_char: Option<i32>,
        pub end_char: Option<i32>,
        pub metadata: Option<serde_json::Value>,
        pub created_at: Option<NaiveDateTime>,
    }

    #[derive(Debug, Insertable, Associations)]
    #[diesel(belongs_to(ProjectDocument, foreign_key = document_id))]
    #[diesel(table_name = document_chunks)]
    pub struct NewDocumentChunk {
        pub document_id: Uuid,
        pub chunk_text: String,
        pub chunk_index: i32,
        pub start_char: Option<i32>,
        pub end_char: Option<i32>,
        pub metadata: Option<serde_json::Value>,
    }

    #[derive(Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, Associations)]
    #[diesel(belongs_to(DocumentChunk, foreign_key = chunk_id))]
    #[diesel(table_name = chunk_embeddings, primary_key(chunk_id))]
    pub struct ChunkEmbedding {
        pub chunk_id: Uuid,
        pub embedding: Option<Vector>,
        pub embedding_model: Option<String>,
        pub created_at: Option<NaiveDateTime>,
    }

    #[derive(Debug, Insertable, Associations)]
    #[diesel(belongs_to(DocumentChunk, foreign_key = chunk_id))]
    #[diesel(table_name = chunk_embeddings)]
    pub struct NewChunkEmbedding {
        pub chunk_id: Uuid,
        pub embedding: Option<Vector>,
        pub embedding_model: Option<String>,
    }

    impl From<Project> for ProjectView {
        fn from(project: Project) -> Self {
            ProjectView {
                id: project.id,
                user_id: project.user_id,
                name: project.name,
                description: project.description,
                instructions: project.instructions,
                created_at: project.created_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                updated_at: project.updated_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            }
        }
    }

    impl From<NewProjectView> for NewProject {
        fn from(view: NewProjectView) -> Self {
            NewProject {
                user_id: 0,
                name: view.name,
                description: view.description,
                instructions: view.instructions,
            }
        }
    }

    impl From<ProjectDocument> for ProjectDocumentView {
        fn from(doc: ProjectDocument) -> Self {
            ProjectDocumentView {
                id: doc.id,
                project_id: doc.project_id,
                filename: doc.filename,
                content: doc.content,
                content_type: doc.content_type,
                file_size: doc.file_size,
                created_at: doc.created_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                updated_at: doc.updated_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            }
        }
    }

    impl From<DocumentChunk> for DocumentChunkView {
        fn from(chunk: DocumentChunk) -> Self {
            DocumentChunkView {
                id: chunk.id,
                document_id: chunk.document_id,
                chunk_text: chunk.chunk_text,
                chunk_index: chunk.chunk_index,
                start_char: chunk.start_char,
                end_char: chunk.end_char,
                metadata: chunk.metadata,
                created_at: chunk.created_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            }
        }
    }

    impl From<ChunkEmbedding> for ChunkEmbeddingView {
        fn from(embedding: ChunkEmbedding) -> Self {
            ChunkEmbeddingView {
                chunk_id: embedding.chunk_id,
                embedding: embedding.embedding.map(|v| v.into()),
                embedding_model: embedding.embedding_model,
                created_at: embedding.created_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            }
        }
    }
}}
