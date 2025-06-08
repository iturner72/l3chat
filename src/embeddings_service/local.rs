#[cfg(feature = "ssr")]
pub mod embeddings_local {
    use candle_core::{DType, Device, Module, Tensor};
    use candle_nn::VarBuilder;
    use tokio_util::sync::CancellationToken;
    use std::{collections::HashMap, convert::Infallible};
    use serde_json::json;
    use log::{error, info};
    use std::sync::OnceLock;
    use thiserror::Error;
    use axum::response::sse::Event;
    use std::error::Error;
    use tokenizers::Tokenizer;

    use crate::components::poasts::Poast;
    use crate::server_fn::{invalidate_poasts_cache, RssProgressUpdate};

    static EMBEDDING_SERVICE: OnceLock<LocalEmbeddingService> = OnceLock::new();

    #[derive(Error, Debug)]
    pub enum EmbeddingError {
        #[error("Tokenizer error: {0}")]
        TokenizerError(#[from] tokenizers::Error),

        #[error("Model error: {0}")]
        ModelError(#[from] candle_core::Error),

        #[error("Initialization error: {0}")]
        InitError(String),

        #[error("Service not initialized")]
        NotInitialized,
    }

    #[derive(Debug)]
    struct Model {
        word_embeddings: candle_nn::Embedding,
        position_embeddings: candle_nn::Embedding,
        token_type_embeddings: candle_nn::Embedding,
        layer_norm: candle_nn::LayerNorm,
        position_ids: Tensor,
    }
    
    impl Model {
        fn new(vb: VarBuilder) -> candle_core::Result<Self> {
            let word_embeddings = candle_nn::embedding(30522, 384, vb.pp("embeddings.word_embeddings"))?;
            let position_embeddings = candle_nn::embedding(512, 384, vb.pp("embeddings.position_embeddings"))?;
            let token_type_embeddings = candle_nn::embedding(2, 384, vb.pp("embeddings.token_type_embeddings"))?;
            let layer_norm = candle_nn::layer_norm(384, 1e-12, vb.pp("embeddings.LayerNorm"))?;

            let position_ids = vb.pp("embeddings").get((1, 512), "position_ids")?.clone();
    
            Ok(Self {
                word_embeddings,
                position_embeddings,
                token_type_embeddings,
                layer_norm,
                position_ids,
            })
        }
    
        fn forward(&self, input_ids: &Tensor) -> candle_core::Result<Tensor> {
            // Ensure input_ids are I64
            let input_ids = if input_ids.dtype() != DType::I64 {
                input_ids.to_dtype(DType::I64)?
            } else {
                input_ids.clone()
            };

            let input_ids = if input_ids.dims().len() == 1 {
                input_ids.unsqueeze(0)?
            } else {
                input_ids
            };

            let batch_size = input_ids.dim(0)?;
            let seq_length = input_ids.dim(1)?;
    
            let embeddings = self.word_embeddings.forward(&input_ids)?;
            
            // Get sequence length and create position ids
            let position_ids = self.position_ids
                .narrow(1, 0, seq_length)?
                .to_dtype(DType::I64)?
                .expand((batch_size, seq_length))?;

            let position_embeddings = self.position_embeddings.forward(&position_ids)?;
            
            // Create token type ids (zeros)
            let token_type_ids = Tensor::zeros_like(&input_ids)?;
            let token_type_embeddings = self.token_type_embeddings.forward(&token_type_ids)?;
            
            // Add all embeddings
            let embeddings = embeddings.add(&position_embeddings)?;
            let embeddings = embeddings.add(&token_type_embeddings)?;
            
            // Apply layer normalization
            let normalized = self.layer_norm.forward(&embeddings)?;
            
            // Mean pooling over sequence length
            normalized.mean(1)
        }
    }

    pub struct LocalEmbeddingService {
        model: Model,
        tokenizer: Tokenizer,
        device: Device,
    }

    impl LocalEmbeddingService {
        pub fn new() -> Result<Self, EmbeddingError> {
            info!("Initializing LocalEmbeddingService");

            let device = Device::Cpu;
            info!("Using device: {device:?}");

            info!("Loading tokenizer");
            let tokenizer = Tokenizer::from_file("models/tokenizer.json")
                .map_err(EmbeddingError::TokenizerError)?;

            unsafe {
                info!("Loading model weights");
                let vb = VarBuilder::from_mmaped_safetensors(
                    &["models/model.safetensors"],
                    DType::F32,
                    &device,
                )
                .map_err(|e| EmbeddingError::InitError(e.to_string()))?;

                info!("Initializing model");
                let model = Model::new(vb).map_err(EmbeddingError::ModelError)?;

                Ok(Self {
                    model,
                    tokenizer,
                    device,
                })
            }
        }

        pub fn get_instance() -> Result<&'static Self, EmbeddingError> {
            EMBEDDING_SERVICE
                .get()
                .ok_or(EmbeddingError::NotInitialized)
        }

        pub fn init() -> Result<(), EmbeddingError> {
            if EMBEDDING_SERVICE.get().is_none() {
                let service = Self::new()?;
                EMBEDDING_SERVICE.set(service).map_err(|_| {
                    EmbeddingError::InitError("Failed to initialize service".to_string())
                })?;
            }
            Ok(())
        }

        pub fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            let encoding = self
                .tokenizer
                .encode(text, true)
                .map_err(EmbeddingError::TokenizerError)?;
    
            let input_ids = encoding.get_ids();
            
            // Create tensor with I64 dtype
            let input_tensor = Tensor::new(
                input_ids,
                &self.device
            ).map_err(EmbeddingError::ModelError)?
            .to_dtype(DType::I64)
            .map_err(EmbeddingError::ModelError)?;
    
            let embedding = self
                .model
                .forward(&input_tensor)
                .map_err(EmbeddingError::ModelError)?
                .squeeze(0)
                .map_err(EmbeddingError::ModelError)?;
            
    
            embedding.to_vec1().map_err(EmbeddingError::ModelError)
        }
    }

    pub async fn generate_local_embeddings(
        progress_sender: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
        cancel_token: CancellationToken,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting embeddings generation process");
        LocalEmbeddingService::init()?;
        let service = LocalEmbeddingService::get_instance()?;
    
        let supabase = crate::supabase::get_client();
        let mut company_states: HashMap<String, RssProgressUpdate> = HashMap::new();
    
        if cancel_token.is_cancelled() {
            info!("Local Embedding generation cancelled before starting");
            return Ok(())
        }
    
        // Get all existing embeddings with pagination
        let mut links_with_embeddings: Vec<String> = Vec::new();
        let page_size = 1000;
        let mut current_page = 0;
    
        loop {
            if cancel_token.is_cancelled() {
                info!("Embeddings retrieval cancelled during pagination");
                return Ok(());
            }
    
            let start = current_page * page_size;
            let end = start + page_size - 1;
            
            info!("Fetching embeddings page {}: range {}-{}", current_page + 1, start, end);
            
            let embeddings_response = supabase
                .from("post_embeddings")
                .select("link,minilm")
                .range(start, end)
                .execute()
                .await?;
    
            let embeddings_text = embeddings_response.text().await?;
            let embeddings_value: serde_json::Value = serde_json::from_str(&embeddings_text)?;
            
            if let serde_json::Value::Array(arr) = embeddings_value {
                if arr.is_empty() {
                    // No more records, exit the loop
                    break;
                }
                
                let page_links: Vec<String> = arr.iter()
                    .filter_map(|v| {
                        let link = v.get("link")?.as_str()?;
                        let minilm = v.get("minilm");
    
                        if minilm.is_some() && !minilm.unwrap().is_null() {
                            Some(link.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                    
                links_with_embeddings.extend(page_links);
                current_page += 1;
            } else {
                // Invalid response format, exit the loop
                break;
            }
        }
    
        info!("Found {} posts with existing minilm embeddings", links_with_embeddings.len());
    
        // Get all posts with pagination
        let mut posts: Vec<Poast> = Vec::new();
        current_page = 0;
    
        loop {
            if cancel_token.is_cancelled() {
                info!("Posts retrieval cancelled during pagination");
                return Ok(());
            }
    
            let start = current_page * page_size;
            let end = start + page_size - 1;
            
            info!("Fetching posts page {}: range {}-{}", current_page + 1, start, end);
            
            let posts_response = supabase
                .from("poasts")
                .select("*")
                .range(start, end)
                .execute()
                .await?;
    
            let posts_text = posts_response.text().await?;
            let posts_value: serde_json::Value = serde_json::from_str(&posts_text)?;
            
            if let serde_json::Value::Array(arr) = posts_value {
                if arr.is_empty() {
                    // No more records, exit the loop
                    break;
                }
                
                let page_posts: Vec<Poast> = arr.iter()
                    .filter_map(|v| {
                        let post: Result<Poast, _> = serde_json::from_value(v.clone());
                        if let Ok(post) = post {
                            if !links_with_embeddings.contains(&post.link) {
                                return Some(post);
                            }
                        }
                        None
                    })
                    .collect();
                    
                posts.extend(page_posts);
                current_page += 1;
            } else {
                // Invalid response format, exit the loop
                break;
            }
        }
    
        info!("Found {} posts needing minilm embeddings", posts.len());
    
        for (index, post) in posts.iter().enumerate() {
            if cancel_token.is_cancelled() {
                info!("Embeddings generation cancelled after {index} posts");
                return Ok(());
            }
    
            let company_progress = company_states
                .entry(post.company.clone())
                .or_insert(RssProgressUpdate {
                    company: post.company.clone(),
                    status: "processing".to_string(),
                    new_posts: 0,
                    skipped_posts: 0,
                    current_post: Some(post.title.clone()),
                });
    
            info!(
                "Processing post {}/{}: '{}' from {}",
                index + 1, posts.len(), post.title, post.company
            );
    
            company_progress.current_post = Some(post.title.clone());
            company_progress.status = "generating embedding".to_string();
    
            progress_sender.send(company_progress.clone().into_event())
                .await
                .map_err(|e| format!("Failed to send progress update: {e}"))?;
    
            let text = format!(
                "{}\n{}\n{}",
                post.title,
                post.summary.as_deref().unwrap_or(""),
                post.description.as_deref().unwrap_or(""),
            );
    
            match service.generate_embedding(&text) {
                Ok(embedding_response) => {
                    company_progress.status = "storing".to_string();
                    progress_sender.send(company_progress.clone().into_event())
                        .await
                        .map_err(|e| format!("Failed to send progress update: {e}"))?;
    
                    let existing = supabase
                        .from("post_embeddings")
                        .select("*")
                        .eq("link", &post.link)
                        .execute()
                        .await;
    
                    let result = match existing {
                        Ok(response) => {
                            let response_text = response.text().await?;
                            let record_exists = !response_text.trim().eq("[]");
    
                            if record_exists {
                                info!("Updating existing record with local embedding for '{}'", post.title);
                                supabase
                                    .from("post_embeddings")
                                    .update(json!({
                                        "minilm": embedding_response
                                    }).to_string())
                                    .eq("link", &post.link)
                                    .execute()
                                    .await
                            } else {
                                info!("Creating new record with local embedding for '{}'", post.title);
                                supabase
                                    .from("post_embeddings")
                                    .insert(json!({
                                        "link": post.link,
                                        "embedding": null,
                                        "minilm": embedding_response
                                    }).to_string())
                                    .execute()
                                    .await
                            }
                        },
                        Err(e) => Err(e)
                    };
    
                    match result {
                        Ok(_) => {
                            company_progress.new_posts += 1;
                            info!("Successfully stored embedding for '{}'", post.title);
                        },
                        Err(e) => {
                            error!("Failed to store embedding for '{}': {}", post.title, e);
                            company_progress.skipped_posts += 1;
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to generate embedding for '{}': {}", post.title, e);
                    company_progress.skipped_posts += 1;
                }
            }
    
            progress_sender.send(company_progress.clone().into_event())
                .await
                .map_err(|e| format!("Failed to send progress update: {e}"))?;
        }
    
        for (_, progress) in company_states.iter_mut() {
            progress.status = "completed".to_string();
            progress.current_post = None;
            progress_sender.send(progress.clone().into_event())
                .await
                .map_err(|e| format!("Failed to send final progress update: {e}"))?;
        }
    
        invalidate_poasts_cache().await.map_err(|e| {
            Box::new(std::io::Error::other(e.to_string())) as Box<dyn Error + Send + Sync>
        })?;
    
        progress_sender
            .send(Ok(Event::default().data("[DONE]")))
            .await
            .map_err(|e| format!("Failed to send completion signal: {e}"))?;
    
        info!("Embeddings generation completed successfully");
        Ok(())
    }

    pub async fn test_single_local_embedding() -> Result<(), Box<dyn std::error::Error>> {
        use crate::components::poasts::Poast;

        LocalEmbeddingService::init()?;
        let service = LocalEmbeddingService::get_instance()?;
        let supabase = crate::supabase::get_client();

        log::info!("Fetching a post from supabase..");
        let response = supabase
            .from("poasts")
            .select("*")
            .limit(1)
            .execute()
            .await?;

        let response_text = response.text().await?;

        let posts: Vec<Poast> = serde_json::from_str(&response_text)?;

        if let Some(post) = posts.first() {
            log::info!("Processing post: {}", post.title);

            let text = format!(
                "{}\n{}\n{}",
                post.title,
                post.summary.as_deref().unwrap_or(""),
                post.description.as_deref().unwrap_or("")
            );
    
            log::info!("Generating local embedding");
            let embedding = service.generate_embedding(&text)?;
            log::info!("Got local embedding with {} dimensions", embedding.len());

            let existing = supabase
                .from("post_embeddings")
                .select("*")
                .eq("link", &post.link)
                .execute()
                .await?;
    
            let existing_text = existing.text().await?;
            let record_exists = !existing_text.trim().eq("[]");
    
            // Prepare the update/insert data
            let data = serde_json::json!({
                "link": post.link,
                "minilm": embedding
            });
            
            log::debug!("Embedding data: {data}");
            
            let result = if record_exists {
                log::info!("Updating existing record with local embedding");
                supabase
                    .from("post_embeddings")
                    .update(data.to_string())
                    .eq("link", &post.link)
                    .execute()
                    .await?
            } else {
                log::info!("Inserting new record with local embedding");
                supabase
                    .from("post_embeddings")
                    .insert(data.to_string())
                    .execute()
                    .await?
            };
            log::info!("Supabase operation status: {}", result.status());
            log::debug!("Supabase response: {}", result.text().await?);
    
            // Verify the embedding was stored correctly
            log::info!("Verifying stored embedding...");
            let verification = supabase
                .from("post_embeddings")
                .select("minilm")
                .eq("link", &post.link)
                .execute()
                .await?;
    
            let verification_text = verification.text().await?;
            log::debug!("Verification response: {verification_text}");
    
            if verification_text.contains("local_embedding") {
                log::info!("Local embedding successfully stored and verified!");
            } else {
                log::error!("Could not verify local embedding storage");
            }
        } else {
            log::info!("No posts found!");
        }
    
        Ok(())
    }
}
