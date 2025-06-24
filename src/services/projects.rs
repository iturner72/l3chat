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
    use serde::{Serialize, Deserialize};
    use std::collections::HashMap;

    use crate::database::db::DbPool;
    use crate::models::projects::*;
    use crate::schema::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DocumentContext {
        pub document_id: Uuid,
        pub filename: String,
        pub content: String,
        pub relevant_chunks: Vec<ChunkMatch>,
        pub file_size: usize,
        pub priority_score: f32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChunkMatch {
        pub chunk_id: Uuid,
        pub chunk_text: String,
        pub similarity: f32,
        pub chunk_index: i32,
        pub start_char: Option<i32>,
        pub end_char: Option<i32>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WorkingContext {
        pub documents: Vec<DocumentContext>,
        pub total_tokens: usize,
        pub summary: Option<String>,
    }

    #[derive(Debug, Clone)]
    pub struct ContextStrategy {
        pub max_total_tokens: usize,
        pub max_full_documents: usize,
        pub small_file_threshold: usize, // Lines
        pub chunk_expansion_lines: usize,
    }

    impl Default for ContextStrategy {
        fn default() -> Self {
            Self {
                max_total_tokens: 100_000, // ~75k tokens leaves room for response
                max_full_documents: 5,
                small_file_threshold: 500, // Lines
                chunk_expansion_lines: 50,
            }
        }
    }

    pub struct EnhancedProjectsService {
        openai: OpenAIClient<async_openai::config::OpenAIConfig>,
        strategy: ContextStrategy,
    }

    impl Default for EnhancedProjectsService {
        fn default() -> Self {
            let openai = OpenAIClient::new();
            Self { 
                openai,
                strategy: ContextStrategy::default(),
            }
        }
    }

    impl EnhancedProjectsService {
        pub fn new() -> Self {
            Default::default()
        }

        pub fn with_strategy(mut self, strategy: ContextStrategy) -> Self {
            self.strategy = strategy;
            self
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

        pub async fn search_project_with_context(
            &self,
            pool: &DbPool,
            project_id: Uuid,
            query: &str,
            limit: i32,
        ) -> Result<WorkingContext, Box<dyn std::error::Error + Send + Sync>> {
            let mut conn = pool.get().await?;

            let query_embedding = self.generate_embedding(query).await?;

            // Find relevant chunks
            let chunk_results = document_chunks::table
                .inner_join(chunk_embeddings::table)
                .inner_join(project_documents::table)
                .filter(project_documents::project_id.eq(project_id))
                .filter(chunk_embeddings::embedding.is_not_null())
                .select((
                    document_chunks::id,
                    document_chunks::chunk_text,
                    chunk_embeddings::embedding.cosine_distance(&query_embedding),
                    document_chunks::document_id,
                    project_documents::filename,
                    document_chunks::chunk_index,
                    document_chunks::start_char,
                    document_chunks::end_char,
                ))
                .order(chunk_embeddings::embedding.cosine_distance(&query_embedding))
                .filter(chunk_embeddings::embedding.cosine_distance(&query_embedding).lt(0.75)) // 1 - 0.25 threshold
                .limit(limit as i64)
                .load::<(Uuid, String, Option<f64>, Uuid, String, i32, Option<i32>, Option<i32>)>(&mut conn)
                .await?;

            // Group chunks by document
            let mut document_chunks: HashMap<Uuid, Vec<ChunkMatch>> = HashMap::new();
            let mut document_info: HashMap<Uuid, String> = HashMap::new();

            for (chunk_id, chunk_text, distance, document_id, filename, chunk_index, start_char, end_char) in chunk_results {
                let similarity = distance.map(|d| 1.0 - d as f32).unwrap_or(0.0);

                document_chunks.entry(document_id).or_default().push(ChunkMatch {
                    chunk_id,
                    chunk_text,
                    similarity,
                    chunk_index,
                    start_char,
                    end_char,
                });

                document_info.insert(document_id, filename);
            }

            // Get full document content for relevant documents
            let document_ids: Vec<Uuid> = document_chunks.keys().cloned().collect();
            let documents = project_documents::table
                .filter(project_documents::id.eq_any(&document_ids))
                .load::<ProjectDocument>(&mut conn)
                .await?;

            // Build document contexts with intelligent content selection
            let mut contexts = Vec::new();
            let mut total_tokens = 0;

            for doc in documents {
                if let Some(chunks) = document_chunks.get(&doc.id) {
                    let file_size = doc.content.lines().count();

                    // calculate priority based on chunk similarity and file characteristics
                    let avg_similarity = chunks.iter().map(|c| c.similarity).sum::<f32>() / chunks.len() as f32;
                    let chunk_density = chunks.len() as f32 / file_size as f32;
                    let priority_score = avg_similarity * 0.7 + chunk_density * 0.3;

                    let content = self.select_document_content(&doc, chunks, file_size)?;
                    let estimated_tokens = content.len() / 4; // Rough token estimation

                    contexts.push(DocumentContext {
                        document_id: doc.id,
                        filename: doc.filename,
                        content,
                        relevant_chunks: chunks.clone(),
                        file_size,
                        priority_score,
                    });

                    total_tokens += estimated_tokens;
                }
            }

            // sort by priority and trim if necessary
            contexts.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap());

            let working_context = self.optimize_context(contexts, total_tokens).await?;

            Ok(working_context)
        }

        // select appropriate content from document based on strategy
        fn select_document_content(
            &self,
            doc: &ProjectDocument,
            chunks: &[ChunkMatch],
            file_size: usize,
        ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            // for small files, return full content
            if file_size <= self.strategy.small_file_threshold {
                return Ok(doc.content.clone());
            }

            // for large files, return expanded chunks
            let lines: Vec<&str> = doc.content.lines().collect();
            let mut selected_ranges = Vec::new();

            for chunk in chunks {
                if let (Some(start_char), Some(end_char)) = (chunk.start_char, chunk.end_char) {
                    // convert char positions to line numbers (approx)
                    let start_line = doc.content[..start_char as usize].lines().count();
                    let end_line = doc.content[..end_char as usize].lines().count();

                    let expanded_start = start_line.saturating_sub(self.strategy.chunk_expansion_lines);
                    let expanded_end = std::cmp::min(
                        end_line + self.strategy.chunk_expansion_lines,
                        lines.len()
                    );

                    selected_ranges.push((expanded_start, expanded_end));
                }
            }

            // merge overlapping ranges and extract content
            selected_ranges.sort_by_key(|r| r.0);
            let merged_ranges = self.merge_ranges(selected_ranges);

            let mut result = String::new();
            for (start, end) in merged_ranges {
                if !result.is_empty() {
                    result.push_str("\n\n... [content omitted] ...\n\n");
                }
                result.push_str(&format!("// Lines {}-{} of {}\n", start + 1, end, doc.filename));
                result.push_str(&lines[start..end].join("\n"));
            }

            Ok(result)
        }

        fn merge_ranges(&self, ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
            if ranges.is_empty() {
                return ranges;
            }

            let mut merged = vec![ranges[0]];

            for current in ranges.into_iter().skip(1) {
                let last_idx = merged.len() - 1;
                if current.0 <= merged[last_idx].1 + 10 { // Merge if close enough
                    merged[last_idx].1 = std::cmp::max(merged[last_idx].1, current.1);
                } else {
                    merged.push(current);
                }
            }

            merged
        }

        async fn optimize_context(
            &self,
            contexts: Vec<DocumentContext>,
            total_tokens: usize,
        ) -> Result<WorkingContext, Box<dyn std::error::Error + Send + Sync>> {
            // if we are within limits, return as is
            if total_tokens <= self.strategy.max_total_tokens && contexts.len() <= self.strategy.max_full_documents {
                return Ok(WorkingContext {
                    documents: contexts,
                    total_tokens,
                    summary: None, 
                });
            }

            // Trim to fit within limits
            let mut current_tokens = 0;
            let mut optimized_contexts = Vec::new();

            for context in contexts.into_iter().take(self.strategy.max_full_documents) {
                let estimated_tokens = context.content.len() / 4;
                if current_tokens + estimated_tokens <= self.strategy.max_total_tokens {
                    current_tokens += estimated_tokens;
                    optimized_contexts.push(context);
                } else {
                    // if we can't fit the full document, include just the chunks
                    let chunk_summary = context.relevant_chunks
                        .iter()
                        .map(|c| format!("// {} (similarity: {:.2})\n{}",
                                context.filename, c.similarity, c.chunk_text))
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    let summary_tokens = chunk_summary.len() / 4;
                    if current_tokens + summary_tokens <= self.strategy.max_total_tokens {
                        optimized_contexts.push(DocumentContext {
                            content: chunk_summary,
                            ..context
                        });
                        current_tokens += summary_tokens;
                    }
                }
            }

            Ok(WorkingContext {
                documents: optimized_contexts,
                total_tokens: current_tokens,
                summary: None,
            })
        }

        /// Create formatted context for LLM
        pub fn format_context_for_llm(&self, working_context: &WorkingContext) -> String {
            let mut formatted = String::new();
            
            formatted.push_str("# Project Documentation Context\n\n");
            
            if let Some(summary) = &working_context.summary {
                formatted.push_str("## Conversation Summary\n");
                formatted.push_str(summary);
                formatted.push_str("\n\n");
            }
            
            formatted.push_str("## Relevant Documents\n\n");
            
            for (i, doc) in working_context.documents.iter().enumerate() {
                formatted.push_str(&format!("### Document {}: {}\n", i + 1, doc.filename));
                formatted.push_str(&format!("**File size:** {} lines | **Priority:** {:.2}\n\n", 
                                          doc.file_size, doc.priority_score));
                
                if !doc.relevant_chunks.is_empty() {
                    let similarities: Vec<String> = doc.relevant_chunks
                        .iter()
                        .map(|c| format!("{:.2}", c.similarity))
                        .collect();
                    formatted.push_str(&format!("**Matching chunks (similarity):** {}\n\n", 
                                               similarities.join(", ")));
                }
                
                formatted.push_str("```\n");
                formatted.push_str(&doc.content);
                formatted.push_str("\n```\n\n");
                formatted.push_str("---\n\n");
            }
            
            formatted.push_str(&format!("\n*Total context: ~{} tokens across {} documents*\n", 
                                      working_context.total_tokens, working_context.documents.len()));
            
            formatted
        }
    }
}

#[cfg(feature = "ssr")]
pub use projects_service::*;
