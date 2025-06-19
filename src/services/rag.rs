#[cfg(feature = "ssr")]
pub mod rag_service {
    use async_openai::{
        config::OpenAIConfig,
        types::{
            ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
            ChatCompletionRequestUserMessage, CreateChatCompletionRequest,
        },
        Client as OpenAIClient,
    };
    use axum::response::sse::Event;
    use futures::StreamExt;
    use log::{debug, error};
    use serde::{Deserialize, Serialize};
    use std::convert::Infallible;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;
    use anyhow::Result;
    use uuid::Uuid;

    use crate::database::db::DbPool;
    use crate::models::projects::ProjectSearchResult;
    use crate::services::projects::ProjectsService;
    use crate::models::conversations::Message;

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct RagResponse {
        pub message_type: String, // "content", "citations", "error", "done", "status"
        pub content: Option<String>,
        pub citations: Option<Vec<DocumentCitation>>,
        pub status: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct DocumentCitation {
        pub filename: String,
        pub chunk_text: String,
        pub similarity: f32,
        pub chunk_index: i32,
    }

    #[derive(Debug, Clone)]
    pub enum LLMProvider {
        OpenAI,
        Anthropic,
    }

    pub struct ProjectRagService {
        openai_client: Option<OpenAIClient<OpenAIConfig>>,
        anthropic_client: Option<reqwest::Client>,
        anthropic_api_key: Option<String>,
        provider: LLMProvider,
        model: String,
        projects_service: ProjectsService,
    }

    impl ProjectRagService {
        pub fn new_openai(model: String) -> Self {
            let client = OpenAIClient::new();
            Self {
                openai_client: Some(client),
                anthropic_client: None,
                anthropic_api_key: None,
                provider: LLMProvider::OpenAI,
                model,
                projects_service: ProjectsService::new(),
            }
        }

        pub fn new_anthropic(model: String) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| "ANTHROPIC_API_KEY must be set")?;
            let client = reqwest::Client::new();
            
            Ok(Self {
                openai_client: None,
                anthropic_client: Some(client),
                anthropic_api_key: Some(api_key),
                provider: LLMProvider::Anthropic,
                model,
                projects_service: ProjectsService::new(),
            })
        }

        pub async fn process_project_query(
            &self,
            pool: &DbPool,
            project_id: Uuid,
            query: String,
            thread_id: &str,
            tx: mpsc::Sender<Result<Event, Infallible>>,
            cancel_token: CancellationToken,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            debug!("Processing project RAG query for project {}: {}", project_id, query);

            // Check for cancellation
            if cancel_token.is_cancelled() {
                return Ok(());
            }

            // Step 1: Send initial status
            self.send_response(&tx, RagResponse {
                message_type: "status".to_string(),
                content: None,
                citations: None,
                status: Some("Searching project documents...".to_string()),
            }).await?;

            // Step 2: Search project documents
            let search_results = match self.projects_service.search_project(pool, project_id, &query, 5).await {
                Ok(results) => results,
                Err(e) => {
                    error!("Failed to search project documents: {}", e);
                    self.send_response(&tx, RagResponse {
                        message_type: "error".to_string(),
                        content: Some("Failed to search project documents".to_string()),
                        citations: None,
                        status: None,
                    }).await?;
                    return Err(e);
                }
            };

            if cancel_token.is_cancelled() {
                return Ok(());
            }

            debug!("Found {} relevant document chunks", search_results.len());

            // Step 3: Send citations if we found relevant documents
            if !search_results.is_empty() {
                let citations: Vec<DocumentCitation> = search_results
                    .iter()
                    .map(|result| DocumentCitation {
                        filename: result.filename.clone(),
                        chunk_text: result.chunk_text.clone(),
                        similarity: result.similarity,
                        chunk_index: result.chunk_index,
                    })
                    .collect();

                self.send_response(&tx, RagResponse {
                    message_type: "citations".to_string(),
                    content: None,
                    citations: Some(citations),
                    status: None,
                }).await?;
            }

            if cancel_token.is_cancelled() {
                return Ok(());
            }

            // Step 4: Get conversation history for context
            let conversation_history = self.get_conversation_history(pool, thread_id).await?;

            // Step 5: Create context and generate response
            let context = self.create_project_context(&search_results);
            
            match self.provider {
                LLMProvider::OpenAI => {
                    if let Some(ref client) = self.openai_client {
                        self.generate_openai_response(
                            query,
                            context,
                            conversation_history,
                            tx,
                            client,
                            cancel_token,
                        ).await?;
                    } else {
                        return Err("OpenAI client not initialized".into());
                    }
                }
                LLMProvider::Anthropic => {
                    if let Some(ref client) = self.anthropic_client {
                        if let Some(ref api_key) = self.anthropic_api_key {
                            self.generate_anthropic_response(
                                query,
                                context,
                                conversation_history,
                                tx,
                                client,
                                api_key,
                                cancel_token,
                            ).await?;
                        } else {
                            return Err("Anthropic API key not found".into());
                        }
                    } else {
                        return Err("Anthropic client not initialized".into());
                    }
                }
            }

            Ok(())
        }

        fn create_project_context(&self, search_results: &[ProjectSearchResult]) -> String {
            if search_results.is_empty() {
                return "No relevant documents found in the project.".to_string();
            }

            let mut context = String::new();
            context.push_str("Here are the most relevant document chunks from the project:\n\n");

            for (i, result) in search_results.iter().enumerate() {
                context.push_str(&format!("Document Chunk {}:\n", i + 1));
                context.push_str(&format!("Filename: {}\n", result.filename));
                context.push_str(&format!("Similarity Score: {:.2}\n", result.similarity));
                context.push_str(&format!("Content:\n{}\n\n", result.chunk_text));
                context.push_str("---\n\n");
            }

            context
        }

        fn create_system_prompt(&self, context: String) -> String {
            format!(
                r#"You are an AI assistant specialized in helping users understand and work with their project documents. 
        You have access to a knowledge base built from the user's uploaded documents, which have been chunked 
        and embedded for semantic search.
        
        KEY INSTRUCTIONS:
        - You are operating in a RAG (Retrieval-Augmented Generation) system
        - Answer questions based ONLY on the provided document chunks
        - If the provided context doesn't contain enough information to answer the question, say so clearly
        - Always reference specific documents when relevant by mentioning the filename
        - Be precise and cite which document chunk supports your answer
        - If no relevant documents are found, respond with "I don't have information about that in the current project documents."
        - Format your response in markdown for better readability
        - Be concise but comprehensive in your explanations
        
        When referencing documents, use this format: **[Filename]** or **[Filename - Chunk N]**
        
        DOCUMENT CONTEXT:
        {context}
        
        Remember: Only use information from the provided document chunks. Do not make assumptions or provide information not contained in the context."#,
            )
        }

        async fn get_conversation_history(
            &self,
            pool: &DbPool,
            thread_id: &str,
        ) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
            crate::components::chat::fetch_message_history(thread_id, pool).await
                .map_err(|e| e.into())
        }

        async fn generate_openai_response(
            &self,
            _query: String,
            context: String,
            history: Vec<Message>,
            tx: mpsc::Sender<Result<Event, Infallible>>,
            client: &OpenAIClient<OpenAIConfig>,
            cancel_token: CancellationToken,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if cancel_token.is_cancelled() {
                return Ok(());
            }

            self.send_response(&tx, RagResponse {
                message_type: "status".to_string(),
                content: None,
                citations: None,
                status: Some("Generating response...".to_string()),
            }).await?;

            let system_prompt = self.create_system_prompt(context);
            let system_message = ChatCompletionRequestSystemMessage {
                content: system_prompt.into(),
                name: None,
            };

            // Convert conversation history to OpenAI format
            let mut messages = vec![ChatCompletionRequestMessage::System(system_message)];
            
            for msg in history {
                let role = msg.role.as_str();
                if let Some(content) = msg.content {
                    match role {
                        "user" => {
                            messages.push(ChatCompletionRequestMessage::User(
                                ChatCompletionRequestUserMessage {
                                    content: content.into(),
                                    name: None,
                                }
                            ));
                        }
                        #[allow(deprecated)]
                        "assistant" => {
                            messages.push(ChatCompletionRequestMessage::Assistant(
                                async_openai::types::ChatCompletionRequestAssistantMessage {
                                    content: Some(content.into()),
                                    name: None,
                                    tool_calls: None,
                                    refusal: None,
                                    audio: None,
                                    function_call: None,
                                }
                            ));
                        }
                        _ => {} // Skip other roles
                    }
                }
            }

            let request = CreateChatCompletionRequest {
                model: self.model.clone(),
                messages,
                stream: Some(true),
                max_completion_tokens: Some(1500),
                temperature: Some(0.7),
                ..Default::default()
            };

            let mut stream = client.chat().create_stream(request).await?;

            while let Some(result) = stream.next().await {
                if cancel_token.is_cancelled() {
                    let _ = tx.send(Ok(Event::default().data("[CANCELLED]"))).await;
                    return Ok(());
                }

                match result {
                    Ok(response) => {
                        for choice in response.choices {
                            if let Some(delta) = choice.delta.content {
                                self.send_response(&tx, RagResponse {
                                    message_type: "content".to_string(),
                                    content: Some(delta),
                                    citations: None,
                                    status: None,
                                }).await?;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error in OpenAI streaming response: {}", e);
                        self.send_response(&tx, RagResponse {
                            message_type: "error".to_string(),
                            content: Some(format!("Error generating response: {}", e)),
                            citations: None,
                            status: None,
                        }).await?;
                        break;
                    }
                }
            }

            // Send completion signal
            self.send_response(&tx, RagResponse {
                message_type: "done".to_string(),
                content: None,
                citations: None,
                status: None,
            }).await?;

            Ok(())
        }

        async fn generate_anthropic_response(
            &self,
            _query: String,
            context: String,
            history: Vec<Message>,
            tx: mpsc::Sender<Result<Event, Infallible>>,
            client: &reqwest::Client,
            api_key: &str,
            cancel_token: CancellationToken,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if cancel_token.is_cancelled() {
                return Ok(());
            }

            self.send_response(&tx, RagResponse {
                message_type: "status".to_string(),
                content: None,
                citations: None,
                status: Some("Generating response with Claude...".to_string()),
            }).await?;

            let system_prompt = self.create_system_prompt(context);

            // Convert conversation history to Anthropic format
            let mut api_messages = Vec::new();
            
            for msg in history {
                if let Some(content) = msg.content {
                    api_messages.push(serde_json::json!({
                        "role": msg.role,
                        "content": content,
                    }));
                }
            }

            let response = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({
                    "model": self.model,
                    "system": system_prompt,
                    "messages": api_messages,
                    "max_tokens": 1500,
                    "stream": true,
                }))
                .send()
                .await?;

            let mut stream = response.bytes_stream();

            while let Some(item) = stream.next().await {
                if cancel_token.is_cancelled() {
                    self.send_response(&tx, RagResponse {
                        message_type: "error".to_string(),
                        content: Some("[CANCELLED]".to_string()),
                        citations: None,
                        status: None,
                    }).await?;
                    return Ok(());
                }

                match item {
                    Ok(bytes) => {
                        let event = String::from_utf8(bytes.to_vec())?;
                        
                        debug!("Raw Anthropic event: {}", event.trim());
                        
                        for line in event.trim().lines() {
                            if line.trim() == "event: message_stop" {
                                debug!("Received message_stop event");
                                self.send_response(&tx, RagResponse {
                                    message_type: "done".to_string(),
                                    content: None,
                                    citations: None,
                                    status: None,
                                }).await?;
                                return Ok(());
                            } else if line.trim().starts_with("data: ") {
                                let json_str = &line.trim()[6..];
                                
                                debug!("Anthropic data line: {}", json_str);
                                
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    if let Some(delta) = parsed["delta"].as_object() {
                                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                            debug!("Extracted Anthropic content: {}", text);
                                            self.send_response(&tx, RagResponse {
                                                message_type: "content".to_string(),
                                                content: Some(text.to_string()),
                                                citations: None,
                                                status: None,
                                            }).await?;
                                        }
                                    }
                                } else {
                                    // Fallback parsing using the same method as regular chat
                                    if let Some(content) = self.extract_text_content(json_str) {
                                        debug!("Fallback extracted Anthropic content: {}", content);
                                        self.send_response(&tx, RagResponse {
                                            message_type: "content".to_string(),
                                            content: Some(content),
                                            citations: None,
                                            status: None,
                                        }).await?;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error in Anthropic streaming response: {}", e);
                        self.send_response(&tx, RagResponse {
                            message_type: "error".to_string(),
                            content: Some(format!("Error generating response: {}", e)),
                            citations: None,
                            status: None,
                        }).await?;
                        break;
                    }
                }
            }

            debug!("Anthropic stream closed");
            Ok(())
        }

        // Add the text extraction method from the working chat.rs
        fn extract_text_content(&self, json_str: &str) -> Option<String> {
            // Find "text":" and extract the content between quotes
            if let Some(start) = json_str.find(r#""text":""#) {
                let content_start = start + 8; // Length of "text":"
                if let Some(content_slice) = json_str.get(content_start..) {
                    // Find the closing quote, handling escaped quotes
                    let mut chars = content_slice.chars();
                    let mut result = String::new();
                    let mut escaped = false;
                    
                    while let Some(ch) = chars.next() {
                        if escaped {
                            match ch {
                                'n' => result.push('\n'),
                                't' => result.push('\t'),
                                'r' => result.push('\r'),
                                '\\' => result.push('\\'),
                                '"' => result.push('"'),
                                _ => {
                                    result.push('\\');
                                    result.push(ch);
                                }
                            }
                            escaped = false;
                        } else if ch == '\\' {
                            escaped = true;
                        } else if ch == '"' {
                            return Some(result);
                        } else {
                            result.push(ch);
                        }
                    }
                }
            }
            None
        }

        async fn send_response(
            &self,
            tx: &mpsc::Sender<Result<Event, Infallible>>,
            response: RagResponse,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let json = serde_json::to_string(&response)?;
            tx.send(Ok(Event::default().data(json))).await
                .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(())
        }
    }

    // Factory function to create appropriate service based on provider
    pub fn create_rag_service(
        provider: &str,
        model: String,
    ) -> Result<ProjectRagService, Box<dyn std::error::Error + Send + Sync>> {
        match provider {
            "openai" => Ok(ProjectRagService::new_openai(model)),
            "anthropic" => ProjectRagService::new_anthropic(model),
            _ => Err(format!("Unsupported LLM provider: {}", provider).into()),
        }
    }
}

#[cfg(feature = "ssr")]
pub use rag_service::*;
