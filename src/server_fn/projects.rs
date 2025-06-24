use cfg_if::cfg_if;
use leptos::prelude::*;
use uuid::Uuid;

use crate::models::projects::*;

#[server(CreateProject, "/api")]
pub async fn create_project(project_data: NewProjectView) -> Result<ProjectView, ServerFnError> {
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::models::projects::{NewProject, Project};
    use crate::schema::projects;
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum ProjectError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
    }

    impl fmt::Display for ProjectError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ProjectError::Pool(e) => write!(f, "Pool error: {e}"),
                ProjectError::Database(e) => write!(f, "Database error: {e}"),
                ProjectError::Unauthorized => write!(f, "Unauthorized"),
            }
        }
    }

    impl From<ProjectError> for ServerFnError {
        fn from(error: ProjectError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    let current_user = get_current_user().await.map_err(|_| ProjectError::Unauthorized)?;
    let user_id = current_user.ok_or(ProjectError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");
    
    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| ProjectError::Pool(e.to_string()))?;

    let mut new_project: NewProject = project_data.into();
    new_project.user_id = user_id;

    let project: Project = diesel::insert_into(projects::table)
        .values(&new_project)
        .get_result(&mut conn)
        .await
        .map_err(ProjectError::Database)?;

    Ok(project.into())
}

#[server(GetUserProjects, "/api")]
pub async fn get_user_projects() -> Result<Vec<ProjectView>, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::models::projects::Project;
    use crate::schema::projects;
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum ProjectError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
    }

    impl fmt::Display for ProjectError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ProjectError::Pool(e) => write!(f, "Pool error: {e}"),
                ProjectError::Database(e) => write!(f, "Database error: {e}"),
                ProjectError::Unauthorized => write!(f, "Unauthorized"),
            }
        }
    }

    impl From<ProjectError> for ServerFnError {
        fn from(error: ProjectError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    let current_user = get_current_user().await.map_err(|_| ProjectError::Unauthorized)?;
    let user_id = current_user.ok_or(ProjectError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| ProjectError::Pool(e.to_string()))?;

    let user_projects: Vec<Project> = projects::table
        .filter(projects::user_id.eq(user_id))
        .order(projects::created_at.desc())
        .load(&mut conn)
        .await
        .map_err(ProjectError::Database)?;

    Ok(user_projects.into_iter().map(ProjectView::from).collect())
}

#[server(UploadDocument, "/api")]
pub async fn upload_document(
    project_id: Uuid,
    filename: String,
    content: String,
    content_type: Option<String>,
) -> Result<ProjectDocumentView, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::models::projects::{NewProjectDocument, ProjectDocument};
    use crate::schema::{projects, project_documents};
    use crate::auth::get_current_user;
    use crate::services::projects::EnhancedProjectsService;

    #[derive(Debug)]
    enum DocumentError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
        ProjectNotFound,
    }

    impl fmt::Display for DocumentError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                DocumentError::Pool(e) => write!(f, "Pool error: {e}"),
                DocumentError::Database(e) => write!(f, "Database error: {e}"),
                DocumentError::Unauthorized => write!(f, "Unauthorized"),
                DocumentError::ProjectNotFound => write!(f, "Project not found"),
            }
        }
    }

    impl From<DocumentError> for ServerFnError {
        fn from(error: DocumentError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    let current_user = get_current_user().await.map_err(|_| DocumentError::Unauthorized)?;
    let user_id = current_user.ok_or(DocumentError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| DocumentError::Pool(e.to_string()))?;

    // Verify project exists and user owns it
    let _project: crate::models::projects::Project = projects::table
        .find(project_id)
        .filter(projects::user_id.eq(user_id))
        .first(&mut conn)
        .await
        .optional()
        .map_err(DocumentError::Database)?
        .ok_or(DocumentError::ProjectNotFound)?;

    let file_size = content.len() as i32;

    let new_document = NewProjectDocument {
        project_id,
        filename: filename.clone(),
        content: content.clone(),
        content_type,
        file_size: Some(file_size),
    };

    let document: ProjectDocument = diesel::insert_into(project_documents::table)
        .values(&new_document)
        .get_result(&mut conn)
        .await
        .map_err(DocumentError::Database)?;

    // Process document asynchronously (chunking and embedding)
    let pool = app_state.pool.clone();
    let document_id = document.id;
    let content_for_processing = content.clone();
    
    tokio::spawn(async move {
        let service = EnhancedProjectsService::new();
        if let Err(e) = service.process_document(&pool, document_id, &content_for_processing).await {
            log::error!("Failed to process document {}: {}", document_id, e);
        } else {
            log::info!("Successfully processed document: {}", document_id);
        }
    });

    Ok(document.into())
}

#[server(GetProjectDocuments, "/api")]
pub async fn get_project_documents(project_id: Uuid) -> Result<Vec<ProjectDocumentView>, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::models::projects::ProjectDocument;
    use crate::schema::{projects, project_documents};
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum DocumentError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
        ProjectNotFound,
    }

    impl fmt::Display for DocumentError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                DocumentError::Pool(e) => write!(f, "Pool error: {e}"),
                DocumentError::Database(e) => write!(f, "Database error: {e}"),
                DocumentError::Unauthorized => write!(f, "Unauthorized"),
                DocumentError::ProjectNotFound => write!(f, "Project not found"),
            }
        }
    }

    impl From<DocumentError> for ServerFnError {
        fn from(error: DocumentError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    let current_user = get_current_user().await.map_err(|_| DocumentError::Unauthorized)?;
    let user_id = current_user.ok_or(DocumentError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| DocumentError::Pool(e.to_string()))?;

    // Verify project exists and user owns it
    let _project: crate::models::projects::Project = projects::table
        .find(project_id)
        .filter(projects::user_id.eq(user_id))
        .first(&mut conn)
        .await
        .optional()
        .map_err(DocumentError::Database)?
        .ok_or(DocumentError::ProjectNotFound)?;

    let documents: Vec<ProjectDocument> = project_documents::table
        .filter(project_documents::project_id.eq(project_id))
        .order(project_documents::created_at.desc())
        .load(&mut conn)
        .await
        .map_err(DocumentError::Database)?;

    Ok(documents.into_iter().map(ProjectDocumentView::from).collect())
}

cfg_if! {
if #[cfg(feature = "ssr")] {
use crate::services::projects::WorkingContext;
#[server(SearchProject, "/api")]
pub async fn search_project(
    project_id: Uuid,
    query: String,
) -> Result<WorkingContext, ServerFnError> {
    use std::fmt;

    use crate::state::AppState;
    use crate::schema::projects;
    use crate::auth::get_current_user;
    use crate::services::projects::EnhancedProjectsService;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    #[derive(Debug)]
    enum SearchError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
        ProjectNotFound,
        SearchError(String),
    }

    impl fmt::Display for SearchError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                SearchError::Pool(e) => write!(f, "Pool error: {e}"),
                SearchError::Database(e) => write!(f, "Database error: {e}"),
                SearchError::Unauthorized => write!(f, "Unauthorized"),
                SearchError::ProjectNotFound => write!(f, "Project not found"),
                SearchError::SearchError(e) => write!(f, "Search error: {e}"),
            }
        }
    }

    impl From<SearchError> for ServerFnError {
        fn from(error: SearchError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    let current_user = get_current_user().await.map_err(|_| SearchError::Unauthorized)?;
    let user_id = current_user.ok_or(SearchError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| SearchError::Pool(e.to_string()))?;

    // Verify project exists and user owns it
    let _project: crate::models::projects::Project = projects::table
        .find(project_id)
        .filter(projects::user_id.eq(user_id))
        .first(&mut conn)
        .await
        .optional()
        .map_err(SearchError::Database)?
        .ok_or(SearchError::ProjectNotFound)?;

    let service = EnhancedProjectsService::new();
    let results = service
        .search_project_with_context(&app_state.pool, project_id, &query, 10)
        .await
        .map_err(|e| SearchError::SearchError(e.to_string()))?;

    Ok(results)
}
}}

#[server(CreateProjectThread, "/api")]
pub async fn create_project_thread(project_id: Uuid) -> Result<String, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;
    use chrono::Utc;

    use crate::state::AppState;
    use crate::models::conversations::Thread;
    use crate::schema::{projects, threads};
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum ThreadError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
        ProjectNotFound,
    }

    impl fmt::Display for ThreadError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ThreadError::Pool(e) => write!(f, "Pool error: {e}"),
                ThreadError::Database(e) => write!(f, "Database error: {e}"),
                ThreadError::Unauthorized => write!(f, "Unauthorized"),
                ThreadError::ProjectNotFound => write!(f, "Project not found"),
            }
        }
    }

    impl From<ThreadError> for ServerFnError {
        fn from(error: ThreadError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    let current_user = get_current_user().await.map_err(|_| ThreadError::Unauthorized)?;
    let user_id = current_user.ok_or(ThreadError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| ThreadError::Pool(e.to_string()))?;

    // Verify project exists and user owns it
    let _project: crate::models::projects::Project = projects::table
        .find(project_id)
        .filter(projects::user_id.eq(user_id))
        .first(&mut conn)
        .await
        .optional()
        .map_err(ThreadError::Database)?
        .ok_or(ThreadError::ProjectNotFound)?;

    let new_thread = Thread {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: Some(Utc::now().naive_utc()),
        updated_at: Some(Utc::now().naive_utc()),
        user_id: Some(user_id),
        parent_thread_id: None,
        branch_point_message_id: None,
        branch_name: None,
        title: None,
        project_id: Some(project_id),
    };

    diesel::insert_into(threads::table)
        .values(&new_thread)
        .execute(&mut conn)
        .await
        .map_err(ThreadError::Database)?;

    Ok(new_thread.id)
}

#[server(DeleteProject, "/api")]
pub async fn delete_project(project_id: Uuid) -> Result<(), ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::{RunQueryDsl, AsyncConnection};
    use std::fmt;

    use crate::state::AppState;
    use crate::schema::{projects, project_documents, document_chunks, chunk_embeddings, threads, messages};
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum DeleteError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
        ProjectNotFound,
    }

    impl fmt::Display for DeleteError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                DeleteError::Pool(e) => write!(f, "Pool error: {e}"),
                DeleteError::Database(e) => write!(f, "Database error: {e}"),
                DeleteError::Unauthorized => write!(f, "Unauthorized"),
                DeleteError::ProjectNotFound => write!(f, "Project not found or access denied"),
            }
        }
    }

    impl From<DeleteError> for ServerFnError {
        fn from(error: DeleteError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }


    impl From<diesel::result::Error> for DeleteError {
        fn from(error: diesel::result::Error) -> Self {
            DeleteError::Database(error)
        }
    }

    let current_user = get_current_user().await.map_err(|_| DeleteError::Unauthorized)?;
    let user_id = current_user.ok_or(DeleteError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| DeleteError::Pool(e.to_string()))?;

    // Use a transaction to ensure all deletions are atomic
    conn.transaction(|conn| {
        Box::pin(async move {
            // First verify the project exists and user owns it
            let project_exists = projects::table
                .find(project_id)
                .filter(projects::user_id.eq(user_id))
                .first::<crate::models::projects::Project>(conn)
                .await
                .optional()
                .map_err(DeleteError::Database)?
                .is_some();

            if !project_exists {
                return Err(DeleteError::ProjectNotFound);
            }

            // Get all document IDs for this project
            let document_ids: Vec<uuid::Uuid> = project_documents::table
                .filter(project_documents::project_id.eq(project_id))
                .select(project_documents::id)
                .load(conn)
                .await
                .map_err(DeleteError::Database)?;

            // Delete embeddings for all chunks in these documents
            if !document_ids.is_empty() {
                let chunk_ids: Vec<uuid::Uuid> = document_chunks::table
                    .filter(document_chunks::document_id.eq_any(&document_ids))
                    .select(document_chunks::id)
                    .load(conn)
                    .await
                    .map_err(DeleteError::Database)?;

                if !chunk_ids.is_empty() {
                    diesel::delete(
                        chunk_embeddings::table.filter(chunk_embeddings::chunk_id.eq_any(&chunk_ids))
                    )
                    .execute(conn)
                    .await
                    .map_err(DeleteError::Database)?;

                    diesel::delete(
                        document_chunks::table.filter(document_chunks::id.eq_any(&chunk_ids))
                    )
                    .execute(conn)
                    .await
                    .map_err(DeleteError::Database)?;
                }

                // Delete all documents for this project
                diesel::delete(
                    project_documents::table.filter(project_documents::project_id.eq(project_id))
                )
                .execute(conn)
                .await
                .map_err(DeleteError::Database)?;
            }

            // Get all thread IDs associated with this project
            let thread_ids: Vec<String> = threads::table
                .filter(threads::project_id.eq(project_id))
                .select(threads::id)
                .load(conn)
                .await
                .map_err(DeleteError::Database)?;

            // Delete all messages in project threads
            if !thread_ids.is_empty() {
                diesel::delete(
                    messages::table.filter(messages::thread_id.eq_any(&thread_ids))
                )
                .execute(conn)
                .await
                .map_err(DeleteError::Database)?;

                // Delete all threads associated with this project
                diesel::delete(
                    threads::table.filter(threads::project_id.eq(project_id))
                )
                .execute(conn)
                .await
                .map_err(DeleteError::Database)?;
            }

            // Finally, delete the project itself
            diesel::delete(projects::table.find(project_id))
                .execute(conn)
                .await
                .map_err(DeleteError::Database)?;

            log::info!("Successfully deleted project {} and all associated data", project_id);
            Ok(())
        })
    })
    .await?;

    Ok(())
}
