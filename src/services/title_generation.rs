#[cfg(feature = "ssr")]
pub mod title_generator {
    use reqwest::Client;
    use serde_json::json;
    use log::{debug, error};
    use std::env;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use serde::{Serialize, Deserialize};
    use axum::response::sse::Event;
    use std::convert::Infallible;
    
    use crate::database::db::DbPool;
    use crate::schema::threads;
    use crate::state::AppState;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TitleUpdate {
        pub thread_id: String,
        pub title: String,
        pub status: String, // "generating", "completed", "error"
    }

    impl TitleUpdate {
        pub fn into_event(self) -> Result<Event, Infallible> {
            Ok(Event::default()
                .data(serde_json::to_string(&self).unwrap_or_default()))
        }
    }

    pub struct TitleGenerationService {
        client: Client,
        api_key: String,
    }

    impl TitleGenerationService {
        pub fn new() -> Self {
            let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
            let client = Client::new();
            TitleGenerationService { client, api_key }
        }

        pub async fn generate_title_streaming(
            &self,
            message_content: &str,
            app_state: &AppState,
            user_id: i32,
            thread_id: &str
        ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            let prompt = format!(
                "Create a concise title (minimum 37 words) for a conversation that starts with this message: \"{}\"\n\nTitle:",
                message_content.chars().take(200).collect::<String>()
            );

            let response = self.client
                .post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": "gpt-4o-mini",
                    "messages": [
                        {
                            "role": "system",
                            "content": "You are a helpful assistant that creates concise, descriptive titles for conversations. Keep titles to 7 words or less. Be specific and relevant to the content."
                        },
                        {
                            "role": "user", 
                            "content": prompt
                        }
                    ],
                    "max_tokens": 20,
                    "temperature": 0.7,
                    "stream": true
                }))
                .send()
                .await?;

            let mut stream = response.bytes_stream();
            let mut accumulated_title = String::new();
            
            use futures::StreamExt;
            
            while let Some(item) = stream.next().await {
                match item {
                    Ok(bytes) => {
                        let chunk = String::from_utf8(bytes.to_vec())?;
                        
                        for line in chunk.trim().lines() {
                            if line.trim() == "data: [DONE]" {
                                break;
                            } else if line.trim().starts_with("data: ") {
                                let json_str = &line.trim()[6..];
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    if let Some(choices) = parsed.get("choices").and_then(|c| c.as_array()) {
                                        if let Some(choice) = choices.get(0) {
                                            if let Some(delta) = choice.get("delta") {
                                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                    accumulated_title.push_str(content);
                                                    
                                                    // Send streaming update with current accumulated title
                                                    let streaming_update = TitleUpdate {
                                                        thread_id: thread_id.to_string(),
                                                        title: accumulated_title.trim().trim_matches('"').to_string(),
                                                        status: "generating".to_string(),
                                                    };
                                                    TitleGenerationService::send_title_update_to_user(app_state, user_id, streaming_update).await;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Stream error: {e}");
                        break;
                    }
                }
            }

            let final_title = accumulated_title.trim().trim_matches('"').to_string();
            
            // Ensure title is not too long
            if final_title.len() > 150 {
                Ok(final_title.chars().take(147).collect::<String>() + "...")
            } else if final_title.is_empty() {
                Ok("New Conversation".to_string())
            } else {
                Ok(final_title)
            }
        }

        pub async fn update_thread_title(
            &self,
            pool: &DbPool,
            thread_id: &str,
            title: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut conn = pool.get().await?;
            
            diesel::update(threads::table.find(thread_id))
                .set(threads::title.eq(title))
                .execute(&mut conn)
                .await?;

            debug!("Updated thread {thread_id} title to: {title}");
            Ok(())
        }

        async fn send_title_update_to_user(
            app_state: &AppState,
            user_id: i32,
            update: TitleUpdate,
        ) {
            let sender = app_state.title_update_senders
                .get(&user_id)
                .map(|sender_ref| sender_ref.value().clone());
            
            if let Some(sender) = sender {
                match sender.send(update.into_event()).await {
                    Ok(_) => {
                        debug!("Successfully sent title update to user {user_id}");
                    }
                    Err(e) => {
                        error!("Failed to send title update to user {user_id}: {e}");
                        
                        app_state.title_update_senders.remove(&user_id);
                    }
                }
            } else {
                error!("No SSE connection found for user {}. Available users: {:?}", 
                    user_id,
                    app_state.title_update_senders.iter()
                        .map(|entry| *entry.key())
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    pub async fn generate_and_update_title_with_sse(
        app_state: AppState,
        user_id: i32,
        thread_id: String,
        message_content: String,
    ) {
        let service = TitleGenerationService::new();
        
        // Send initial "generating" status immediately
        let generating_update = TitleUpdate {
            thread_id: thread_id.clone(),
            title: "Generating title...".to_string(),
            status: "generating".to_string(),
        };
        TitleGenerationService::send_title_update_to_user(&app_state, user_id, generating_update).await;
        
        // Use streaming title generation
        match service.generate_title_streaming(&message_content, &app_state, user_id, &thread_id).await {
            Ok(title) => {
                if let Err(e) = service.update_thread_title(&app_state.pool, &thread_id, &title).await {
                    error!("Failed to update thread title: {e}");
                    
                    // Send error status
                    let error_update = TitleUpdate {
                        thread_id: thread_id.clone(),
                        title: "Error generating title".to_string(),
                        status: "error".to_string(),
                    };
                    TitleGenerationService::send_title_update_to_user(&app_state, user_id, error_update).await;
                } else {
                    // Send completed status with the final title
                    let completed_update = TitleUpdate {
                        thread_id: thread_id.clone(),
                        title: title.clone(),
                        status: "completed".to_string(),
                    };
                    TitleGenerationService::send_title_update_to_user(&app_state, user_id, completed_update).await;
                }
            }
            Err(e) => {
                error!("Failed to generate title for thread {thread_id}: {e}");
                
                // Send error status
                let error_update = TitleUpdate {
                    thread_id: thread_id.clone(),
                    title: "Error generating title".to_string(),
                    status: "error".to_string(),
                };
                TitleGenerationService::send_title_update_to_user(&app_state, user_id, error_update).await;
            }
        }
    }
}

#[cfg(feature = "ssr")]
pub use title_generator::*;
