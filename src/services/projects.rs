#[cfg(feature = "ssr")]
pub mod projects_service {
    use pgvector::{Vector, VectorExpressionMethods};
    use async_openai::{
        types::{CreateEmbeddingRequestArgs, EmbeddingInput},
        Client as OpenAIClient,
    };
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use uuid::Uuid;

    use crate::database::db::DbPool;
    use crate::models::projects::*;
    use crate::schema::*;

    pub struct ProjectsService {
        openai: OpenAIClient<async_openai::config::OpenAIConfig>,
    }

    impl ProjectsService {
        pub fn new() -> Self {
            let openai = OpenAIClient::new();
            Self { openai }
        }

        pub fn chunk_text(&self, text: &str, chunk_size: usize, overlap: usize) -> Vec<(String, usize, usize)> {
            let mut chunks = Vec::new();
            let chars: Vec<char> = text.chars().collect();
            let mut start = 0;
    
            while start < chars.len() {
                let end = std::cmp::min(start + chunk_size, chars.len());
                let chunk_text: String = chars[start..end].iter().collect();
    
                chunks.push((chunk_text, start, end));
    
                if end >= chars.len() {
                    break;
                }
    
                start = if end > overlap { end - overlap } else { end };
            }
    
            chunks
        }

        pub async fn generate_embedding(&self, text: &str) -> Result<Vector, Box<dyn std::error::Error + Send + Sync>> {
            let response = self.openai
                .embeddings()
                .create(CreateEmbeddingRequestArgs::default()
                    .model("text-embedding-3-small")
                    .input(EmbeddingInput::String(text.to_string()))
                    .build()?)
                .await?;
    
            Ok(response.data[0].embedding.clone().into())
        }
    
        pub async fn process_document(
            &self,
            pool: &DbPool,
            document_id: Uuid,
            content: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut conn = pool.get().await?;
    
            diesel::delete(
                chunk_embeddings::table
                    .filter(chunk_embeddings::chunk_id.eq_any(
                        document_chunks::table
                            .select(document_chunks::id)
                            .filter(document_chunks::document_id.eq(document_id))
                    ))
            )
            .execute(&mut conn)
            .await?;
    
            diesel::delete(
                document_chunks::table.filter(document_chunks::document_id.eq(document_id))
            )
            .execute(&mut conn)
            .await?;
    
            let chunks = self.chunk_text(content, 1000, 200);
    
            for (index, (chunk_text, start_char, end_char)) in chunks.into_iter().enumerate() {
                if chunk_text.trim().len() < 10 {
                    continue;
                }
    
                let new_chunk = NewDocumentChunk {
                    document_id,
                    chunk_text: chunk_text.clone(),
                    chunk_index: index as i32,
                    start_char: Some(start_char as i32),
                    end_char: Some(end_char as i32),
                    metadata: Some(serde_json::json!({
                        "word_count": chunk_text.split_whitespace().count()
                    })),
                };
    
                let chunk: DocumentChunk = diesel::insert_into(document_chunks::table)
                    .values(&new_chunk)
                    .get_result(&mut conn)
                    .await?;
    
                let embedding = self.generate_embedding(&chunk_text).await?;
    
                let new_embedding = NewChunkEmbedding {
                    chunk_id: chunk.id,
                    embedding: Some(embedding),
                    embedding_model: Some("text-embedding-3-small".to_string()),
                };
    
                diesel::insert_into(chunk_embeddings::table)
                    .values(&new_embedding)
                    .execute(&mut conn)
                    .await?;
            }
    
            Ok(())
        }

        pub async fn search_project(
            &self,
            pool: &DbPool,
            project_id: Uuid,
            query: &str,
            limit: i32,
        ) -> Result<Vec<ProjectSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
            let mut conn = pool.get().await?;

            let query_embedding = self.generate_embedding(query).await?;

            let results = document_chunks::table
                .inner_join(chunk_embeddings::table.on(document_chunks::id.eq(chunk_embeddings::chunk_id)))
                .inner_join(project_documents::table.on(document_chunks::document_id.eq(project_documents::id)))
                .filter(project_documents::project_id.eq(project_id))
                .filter(chunk_embeddings::embedding.is_not_null())
                .select((
                    document_chunks::id,
                    document_chunks::chunk_text,
                    chunk_embeddings::embedding.cosine_distance(&query_embedding),
                    document_chunks::document_id,
                    project_documents::filename,
                    document_chunks::chunk_index,
                ))
                .order(chunk_embeddings::embedding.cosine_distance(&query_embedding))
                .filter(chunk_embeddings::embedding.cosine_distance(&query_embedding).lt(0.75)) // 1 - 0.25 threshold
                .limit(limit as i64)
                .load::<(Uuid, String, Option<f64>, Uuid, String, i32)>(&mut conn)
                .await?;

            let search_results = results
                .into_iter()
                .map(|(chunk_id, chunk_text, distance, document_id, filename, chunk_index)| {
                    let similarity = distance.map(|d| 1.0 - d as f32).unwrap_or(0.0);
                    ProjectSearchResult {
                        chunk_id,
                        chunk_text,
                        similarity,
                        document_id,
                        filename,
                        chunk_index,
                    }
                })
                .collect();

            Ok(search_results)
        }
    
        pub async fn get_project_context(
            &self,
            pool: &DbPool,
            project_id: Uuid,
            query: &str,
        ) -> Result<String, Box<dyn std::error::Error + Send + Sync >> {
            let mut conn = pool.get().await?;
    
            let project: Project = projects::table
                .find(project_id)
                .first(&mut conn)
                .await?;
    
            let search_results = self.search_project(pool, project_id, query, 5).await?;

            log::info!("found results: {search_results:?}");
    
            let mut context = String::new();
    
            if let Some(instructions) = &project.instructions {
                context.push_str("PROJECT INSTRUCTIONS:\n");
                context.push_str(instructions);
                context.push_str("\n\n");
            }
    
            if !search_results.is_empty() {
                context.push_str("RELEVANT CONTEXT:\n");
                for result in search_results {
                    context.push_str(&format!(
                        "From {}: {}\n\n",
                        result.filename,
                        result.chunk_text
                    ));
                }
            }
    
            Ok(context)
        }
    }
}

#[cfg(feature = "ssr")]
pub use projects_service::*;
