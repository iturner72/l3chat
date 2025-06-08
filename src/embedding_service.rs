#[cfg(feature = "ssr")]
pub mod embeddings {

use async_openai::{
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
    Client,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use std::{collections::HashMap, convert::Infallible};
use axum::response::sse::Event;
use std::error::Error;
use log::{info, error};

use crate::components::poasts::Poast;
use crate::server_fn::{invalidate_poasts_cache, RssProgressUpdate};

// will need to run this in supabase
const _MIGRATION_SQL: &str = r#"
CREATE TABLE post_embeddings (
    link TEXT PRIMARY KEY REFERENCES poasts(link),
    embedding vector(1536)
);
CREATE INDEX ON post_embeddings USING ivfflat (embedding vector_cosine_ops);
"#;

pub async fn generate_embeddings(
    progress_sender: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
    cancel_token: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting embeddings generation process");
    let openai = Client::new();
    let supabase = crate::supabase::get_client();
    let mut company_states: HashMap<String, RssProgressUpdate> = HashMap::new();

    if cancel_token.is_cancelled() {
        info!("Embedding generation cancelled before starting");
        return Ok(())
    }

    // First get all existing embeddings with pagination
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
            .select("link,embedding")
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
                    let embedding = v.get("embedding");

                    if embedding.is_some() && !embedding.unwrap().is_null() {
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

    info!("Found {} posts with existing OpenAI embeddings", links_with_embeddings.len());

    // Now get all posts with pagination
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
        
        if current_page == 0 {
            info!("First page Supabase posts response: {posts_text}");
        } else {
            info!("Retrieved page {} of posts", current_page + 1);
        }

        let posts_value: serde_json::Value = serde_json::from_str(&posts_text)?;
        
        if let serde_json::Value::Array(arr) = posts_value {
            if arr.is_empty() {
                // No more records, exit the loop
                break;
            }
            
            let page_posts: Vec<Poast> = arr.iter()
                .filter_map(|v| {
                    let result: Result<Poast, _> = serde_json::from_value(v.clone());
                    if let Ok(post) = result {
                        if !links_with_embeddings.contains(&post.link) {
                            return Some(post);
                        }
                    } else if let Err(ref e) = result {
                        error!("Failed to parse post: {e}");
                    }
                    None
                })
                .collect();
                
            posts.extend(page_posts);
            current_page += 1;
        } else {
            error!("Expected array response from Supabase, got: {posts_text}");
            break;
        }
    }

    info!("Found {} posts needing embeddings across {} pages", posts.len(), current_page);

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

        match openai
            .embeddings()
            .create(CreateEmbeddingRequestArgs::default()
                .model("text-embedding-3-small")
                .input(EmbeddingInput::String(text))
                .build()?)
            .await
        {
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
                            info!("Updating existing record with embedding for '{}'", post.title);
                            supabase
                                .from("post_embeddings")
                                .update(json!({
                                    "embedding": embedding_response.data[0].embedding
                                }).to_string())
                                .eq("link", &post.link)
                                .execute()
                                .await
                        } else {
                            info!("Creating new record with embedding for '{}'", post.title);
                            supabase
                                .from("post_embeddings")
                                .insert(json!({
                                    "link": post.link,
                                    "embedding": embedding_response.data[0].embedding,
                                    "minilm": null
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

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
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


#[cfg(feature = "ssr")]
// code smell will call this with an CLI binary (first time i've worked with vector extension in
// supabase, it worked first try) will remove this, or consider alternative methods for calling.
//
// I might keep this for testing new models later.
pub async fn test_single_embedding() -> Result<(), Box<dyn std::error::Error>> {
    use async_openai::{
        types::{CreateEmbeddingRequestArgs, EmbeddingInput},
        Client,
    };
    use crate::components::poasts::Poast;
    
    let openai = Client::new();
    let supabase = crate::supabase::get_client();

    log::info!("Fetching a post from Supabase...");
    let response = supabase
        .from("poasts")
        .select("*")
        .limit(1)
        .execute()
        .await?;

    let response_text = response.text().await?;
    log::debug!("Supabase response: {response_text}");

    let posts: Vec<Poast> = serde_json::from_str(&response_text)?;
    
    if let Some(post) = posts.first() {
        log::info!("Processing post: {}", post.title);
        
        let text = format!(
            "{}\n{}\n{}",
            post.title,
            post.summary.as_deref().unwrap_or(""),
            post.description.as_deref().unwrap_or("")
        );

        log::info!("Getting embedding from OpenAI");
        let embedding_response = openai
            .embeddings()
            .create(CreateEmbeddingRequestArgs::default()
                .model("text-embedding-3-small")
                .input(EmbeddingInput::String(text))
                .build()?)
            .await?;

        let embedding = embedding_response.data[0].embedding.clone();
        log::info!("Got embedding with {} dimensions", embedding.len());

        log::info!("Inserting embedding into Supabase");
        let insert_data = serde_json::json!({
            "link": post.link,
            "embedding": embedding
        });
        
        log::debug!("Insert data: {insert_data}");
        
        let result = supabase
            .from("post_embeddings")
            .insert(insert_data.to_string())
            .execute()
            .await?;

        log::info!("Insertion result status: {}", result.status());
        log::debug!("Insertion response: {}", result.text().await?);
    } else {
        log::info!("No posts found!");
    }

    Ok(())
}

}
