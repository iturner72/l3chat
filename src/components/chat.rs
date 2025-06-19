use cfg_if::cfg_if;
use leptos::{prelude::*, task::spawn_local};
use log::{info, error};
use serde::{Serialize, Deserialize};
use urlencoding;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::JsFuture;
use web_sys::{EventSource, MessageEvent, ErrorEvent, HtmlElement};
use chrono::Utc;

use crate::{auth::get_current_user, models::conversations::{NewMessageView, PendingMessage}};
use crate::components::toast::Toast;
use crate::types::StreamResponse;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RagResponse {
    pub message_type: String,
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

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::response::sse::Event;
        use anyhow::{anyhow, Error};
        use reqwest::Client;
        use regex::Regex;
        use std::env;
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use serde_json::Value;
        use tokio::sync::mpsc;
        use futures::stream::{Stream, StreamExt};
        use tokio_util::sync::CancellationToken;
        use std::convert::Infallible;
        use log::debug;

        use crate::database::db::DbPool;
        use crate::models::conversations::Message;
        use crate::services::rag::create_rag_service;

        pub struct SseStream {
            pub receiver: mpsc::Receiver<Result<Event, anyhow::Error>>,
        }

        impl Stream for SseStream {
            type Item = Result<Event, anyhow::Error>;

            fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                self.receiver.poll_recv(cx)
            }
        }

        #[derive(Clone)]
        pub struct OpenAIService {
            client: Client,
            api_key: String,
            model: String,
        }

        #[derive(Clone)]
        pub struct AnthropicService {
            client: Client,
            api_key: String,
            model: String,
        }

        impl AnthropicService {
            pub fn new(model: String) -> Self {
                let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set.");
                let client = Client::new();
                AnthropicService { client, api_key, model }
            }
        
            pub async fn send_message_with_context_cancellable(
                &self,
                pool: &DbPool,
                thread_id: &str,
                context: &str,
                tx: mpsc::Sender<Result<Event, Infallible>>,
                cancel_token: CancellationToken,
            ) -> Result<(), anyhow::Error> {
                debug!("Sending message to Anthropic API with project context (cancellable)");
                debug!("Current thread id: {thread_id}");
        
                // Check for cancellation before starting
                if cancel_token.is_cancelled() {
                    info!("Anthropic message cancelled before starting");
                    return Ok(());
                }
        
                let mut history = fetch_message_history(thread_id, pool).await?;
                
                // Inject context before the last user message
                if let Some(last_message) = history.last_mut() {
                    if last_message.role == "user" {
                        if let Some(ref mut content) = last_message.content {
                            *content = format!("{context}\n\nUser Query: {content}");
                        }
                    }
                }
        
                // Check for cancellation before API call
                if cancel_token.is_cancelled() {
                    info!("Anthropic message cancelled before API call");
                    return Ok(());
                }
        
                let api_messages = history.into_iter()
                    .map(|msg| serde_json::json!({
                        "role": msg.role,
                        "content": msg.content.unwrap_or_default(),
                    }))
                    .collect::<Vec<_>>();
        
                let response = self.client.post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", self.api_key.to_string())
                    .header("anthropic-version", "2023-06-01")
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": self.model,
                        "messages": api_messages,
                        "max_tokens": 1360,
                        "stream": true,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow!("Failed to send message: {}", e))?;
        
                let mut stream = response.bytes_stream();
        
                while let Some(item) = stream.next().await {
                    // Check for cancellation in each iteration
                    if cancel_token.is_cancelled() {
                        info!("Anthropic message stream cancelled during processing");
                        let _ = tx.send(Ok(Event::default().data("[CANCELLED]"))).await;
                        return Ok(());
                    }
        
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec())
                                .map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            
                            debug!("Raw event: {}", event.trim());
                            
                            for line in event.trim().lines() {
                                if line.trim() == "event: message_stop" {
                                    debug!("Received message_stop event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    return Ok(());
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    
                                    if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(delta) = parsed["delta"].as_object() {
                                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                                debug!("Extracted content: {}", text);
                                                tx.send(Ok(Event::default().data(text))).await.ok();
                                            }
                                        }
                                    } else {
                                        if let Some(content) = extract_text_content(json_str) {
                                            debug!("Fallback extracted content: {}", content);
                                            tx.send(Ok(Event::default().data(content))).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to process stream: {e}");
                            let error_event = Event::default().data(format!("Error: Failed to process stream: {e}"));
                            tx.send(Ok(error_event)).await.ok();
                            break;
                        }
                    }
                }
        
                debug!("Stream closed");
                Ok(())
            }
        
            pub async fn send_message_cancellable(
                &self,
                pool: &DbPool,
                thread_id: &str,
                tx: mpsc::Sender<Result<Event, Infallible>>,
                cancel_token: CancellationToken,
            ) -> Result<(), anyhow::Error> {
                debug!("Sending message to Anthropic API (cancellable)");
                debug!("Current thread id: {thread_id}");
        
                // Check for cancellation before starting
                if cancel_token.is_cancelled() {
                    info!("Anthropic message cancelled before starting");
                    return Ok(());
                }
        
                let history = fetch_message_history(thread_id, pool).await?;
        
                // Check for cancellation before API call
                if cancel_token.is_cancelled() {
                    info!("Anthropic message cancelled before API call");
                    return Ok(());
                }
        
                let api_messages = history.into_iter()
                    .map(|msg| serde_json::json!({
                        "role": msg.role,
                        "content": msg.content.unwrap_or_default(),
                    }))
                    .collect::<Vec<_>>();
        
                let response = self.client.post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", self.api_key.to_string())
                    .header("anthropic-version", "2023-06-01")
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": self.model,
                        "messages": api_messages,
                        "max_tokens": 1360,
                        "stream": true,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow!("Failed to send message: {}", e))?;
        
                let mut stream = response.bytes_stream();
        
                while let Some(item) = stream.next().await {
                    // Check for cancellation in each iteration
                    if cancel_token.is_cancelled() {
                        info!("Anthropic message stream cancelled during processing");
                        let _ = tx.send(Ok(Event::default().data("[CANCELLED]"))).await;
                        return Ok(());
                    }
        
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec())
                                .map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            
                            debug!("Raw event: {}", event.trim());
                            
                            for line in event.trim().lines() {
                                if line.trim() == "event: message_stop" {
                                    debug!("Received message_stop event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    return Ok(());
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    
                                    if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(delta) = parsed["delta"].as_object() {
                                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                                debug!("Extracted content: {}", text);
                                                tx.send(Ok(Event::default().data(text))).await.ok();
                                            }
                                        }
                                    } else {
                                        if let Some(content) = extract_text_content(json_str) {
                                            debug!("Fallback extracted content: {}", content);
                                            tx.send(Ok(Event::default().data(content))).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to process stream: {e}");
                            let error_event = Event::default().data(format!("Error: Failed to process stream: {e}"));
                            tx.send(Ok(error_event)).await.ok();
                            break;
                        }
                    }
                }
        
                debug!("Stream closed");
                Ok(())
            }
        }

        pub fn extract_text_content(json_str: &str) -> Option<String> {
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

        impl OpenAIService {
            pub fn new(model: String) -> Self {
                let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set.");
                let client = Client::new();
                OpenAIService { client, api_key, model }
            }
        
            pub async fn send_message_with_context_cancellable(
                &self,
                pool: &DbPool,
                thread_id: &str,
                context: &str,
                tx: mpsc::Sender<Result<Event, Infallible>>,
                cancel_token: CancellationToken,
            ) -> Result<(), anyhow::Error> {
                debug!("Sending message to OpenAI API with project context (cancellable)");
                debug!("Current thread id: {thread_id}");
        
                // Check for cancellation before starting
                if cancel_token.is_cancelled() {
                    info!("OpenAI message cancelled before starting");
                    return Ok(());
                }
        
                let mut history = fetch_message_history(thread_id, pool).await?;
        
                // Inject context before the last user message
                if let Some(last_message) = history.last_mut() {
                    if last_message.role == "user" {
                        if let Some(ref mut content) = last_message.content {
                            *content = format!("{}\n\nUser Query: {}", context, content);
                        }
                    }
                }
        
                // Check for cancellation before API call
                if cancel_token.is_cancelled() {
                    info!("OpenAI message cancelled before API call");
                    return Ok(());
                }
        
                let api_messages = history.into_iter()
                    .map(|msg| serde_json::json!({
                        "role": msg.role,
                        "content": msg.content.unwrap_or_default(),
                    }))
                    .collect::<Vec<_>>();
        
                let response = self.client.post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": self.model,
                        "messages": api_messages,
                        "max_tokens": 1360,
                        "stream": true,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow!("Failed to send message: {}", e))?;
        
                let mut stream = response.bytes_stream();
                
                while let Some(item) = stream.next().await {
                    // Check for cancellation in each iteration
                    if cancel_token.is_cancelled() {
                        info!("OpenAI message stream cancelled during processing");
                        let _ = tx.send(Ok(Event::default().data("[CANCELLED]"))).await;
                        return Ok(());
                    }
        
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec())
                                .map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            
                            debug!("Raw event: {}", event.trim());
                            
                            for line in event.trim().lines() {
                                if line.trim() == "data: [DONE]" {
                                    debug!("Received [DONE] event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    return Ok(());
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    
                                    if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(choices) = parsed["choices"].as_array() {
                                            if let Some(first_choice) = choices.first() {
                                                if let Some(delta) = first_choice["delta"].as_object() {
                                                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                        debug!("Extracted content: {}", content);
                                                        tx.send(Ok(Event::default().data(content))).await.ok();
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        if let Some(content) = extract_content_with_escaping(json_str) {
                                            debug!("Fallback extracted content: {}", content);
                                            tx.send(Ok(Event::default().data(content))).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to process stream: {e}");
                            let error_event = Event::default().data(format!("Error: Failed to process stream: {e}"));
                            tx.send(Ok(error_event)).await.ok();
                            break;
                        }
                    }
                }
        
                debug!("Stream closed");
                Ok(())
            }
        
            pub async fn send_message_cancellable(
                &self,
                pool: &DbPool,
                thread_id: &str,
                tx: mpsc::Sender<Result<Event, Infallible>>,
                cancel_token: CancellationToken,
            ) -> Result<(), anyhow::Error> {
                debug!("Sending message to OpenAI API (cancellable)");
                debug!("Current thread id: {thread_id}");
        
                // Check for cancellation before starting
                if cancel_token.is_cancelled() {
                    info!("OpenAI message cancelled before starting");
                    return Ok(());
                }
        
                let history = fetch_message_history(thread_id, pool).await?;
        
                // Check for cancellation before API call
                if cancel_token.is_cancelled() {
                    info!("OpenAI message cancelled before API call");
                    return Ok(());
                }
        
                let api_messages = history.into_iter()
                    .map(|msg| serde_json::json!({
                        "role": msg.role,
                        "content": msg.content.unwrap_or_default(),
                    }))
                    .collect::<Vec<_>>();
        
                let response = self.client.post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": self.model,
                        "messages": api_messages,
                        "max_tokens": 1360,
                        "stream": true,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow!("Failed to send message: {}", e))?;
        
                let mut stream = response.bytes_stream();
        
                while let Some(item) = stream.next().await {
                    // Check for cancellation in each iteration
                    if cancel_token.is_cancelled() {
                        info!("OpenAI message stream cancelled during processing");
                        let _ = tx.send(Ok(Event::default().data("[CANCELLED]"))).await;
                        return Ok(());
                    }
        
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec())
                                .map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            
                            debug!("Raw event: {}", event.trim());
                            
                            for line in event.trim().lines() {
                                if line.trim() == "data: [DONE]" {
                                    debug!("Received [DONE] event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    return Ok(());
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    
                                    if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(choices) = parsed["choices"].as_array() {
                                            if let Some(first_choice) = choices.first() {
                                                if let Some(delta) = first_choice["delta"].as_object() {
                                                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                        debug!("Extracted content: {}", content);
                                                        tx.send(Ok(Event::default().data(content))).await.ok();
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        if let Some(content) = extract_content_with_escaping(json_str) {
                                            debug!("Fallback extracted content: {}", content);
                                            tx.send(Ok(Event::default().data(content))).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to process stream: {e}");
                            let error_event = Event::default().data(format!("Error: Failed to process stream: {e}"));
                            tx.send(Ok(error_event)).await.ok();
                            break;
                        }
                    }
                }
        
                debug!("Stream closed");
                Ok(())
            }
        
            // Keep the original non-cancellable methods for backward compatibility
            pub async fn send_message_with_context(
                &self,
                pool: &DbPool,
                thread_id: &str,
                context: &str,
                tx: mpsc::Sender<Result<Event, std::convert::Infallible>>
            ) -> Result<(), anyhow::Error> {
                debug!("Sending message to OpenAI API with project context");
                debug!("Current thread id: {thread_id}");
        
                let mut history = fetch_message_history(thread_id, pool).await?;
        
                // Inject context before the last user message
                if let Some(last_message) = history.last_mut() {
                    if last_message.role == "user" {
                        if let Some(ref mut content) = last_message.content {
                            *content = format!("{}\n\nUser Query: {}", context, content);
                        }
                    }
                }
        
                let api_messages = history.into_iter()
                    .map(|msg| serde_json::json!({
                        "role": msg.role,
                        "content": msg.content.unwrap_or_default(),
                    }))
                    .collect::<Vec<_>>();
        
                let response = self.client.post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": self.model,
                        "messages": api_messages,
                        "max_tokens": 1360,
                        "stream": true,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow!("Failed to send message: {}", e))?;
        
                let mut stream = response.bytes_stream();
                
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec())
                                .map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            
                            debug!("Raw event: {}", event.trim());
                            
                            for line in event.trim().lines() {
                                if line.trim() == "data: [DONE]" {
                                    debug!("Received [DONE] event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    return Ok(());
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    
                                    if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(choices) = parsed["choices"].as_array() {
                                            if let Some(first_choice) = choices.first() {
                                                if let Some(delta) = first_choice["delta"].as_object() {
                                                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                        debug!("Extracted content: {}", content);
                                                        tx.send(Ok(Event::default().data(content))).await.ok();
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        if let Some(content) = extract_content_with_escaping(json_str) {
                                            debug!("Fallback extracted content: {}", content);
                                            tx.send(Ok(Event::default().data(content))).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to process stream: {e}");
                            let error_event = Event::default().data(format!("Error: Failed to process stream: {e}"));
                            tx.send(Ok(error_event)).await.ok();
                            break;
                        }
                    }
                }
        
                debug!("Stream closed");
                Ok(())
            }
        
            pub async fn send_message(
                &self,
                pool: &DbPool,
                thread_id: &str,
                tx: mpsc::Sender<Result<Event, std::convert::Infallible>>
            ) -> Result<(), anyhow::Error> {
                debug!("Sending message to OpenAI API");
                debug!("Current thread id: {thread_id}");
        
                let history = fetch_message_history(thread_id, pool).await?;
        
                let api_messages = history.into_iter()
                    .map(|msg| serde_json::json!({
                        "role": msg.role,
                        "content": msg.content.unwrap_or_default(),
                    }))
                    .collect::<Vec<_>>();
        
                let response = self.client.post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": self.model,
                        "messages": api_messages,
                        "max_tokens": 1360,
                        "stream": true,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow!("Failed to send message: {}", e))?;
        
                let mut stream = response.bytes_stream();
        
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec())
                                .map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            
                            debug!("Raw event: {}", event.trim());
                            for line in event.trim().lines() {
                                if line.trim() == "data: [DONE]" {
                                    debug!("Received [DONE] event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    return Ok(());
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    
                                    // Parse the JSON properly instead of using regex
                                    if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(choices) = parsed["choices"].as_array() {
                                            if let Some(first_choice) = choices.first() {
                                                if let Some(delta) = first_choice["delta"].as_object() {
                                                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                        debug!("Extracted content: {}", content);
                                                        tx.send(Ok(Event::default().data(content))).await.ok();
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        if let Some(content) = extract_content_with_escaping(json_str) {
                                            debug!("Fallback extracted content: {}", content);
                                            tx.send(Ok(Event::default().data(content))).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to process stream: {e}");
                            let error_event = Event::default().data(format!("Error: Failed to process stream: {e}"));
                            tx.send(Ok(error_event)).await.ok();
                            break;
                        }
                    }
                }
        
                debug!("Stream closed");
                Ok(())
            }
        }

        /// Fallback function to extract content with proper escape handling
        fn extract_content_with_escaping(json_str: &str) -> Option<String> {
            // More sophisticated regex that handles escaped quotes
            let re = Regex::new(r#""content":"((?:[^"\\]|\\.)*)""#).unwrap();
            
            if let Some(captures) = re.captures(json_str) {
                if let Some(content_match) = captures.get(1) {
                    let content = content_match.as_str();
                    
                    // Unescape the JSON string
                    let unescaped = content
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\")
                        .replace("\\n", "\n")
                        .replace("\\r", "\r")
                        .replace("\\t", "\t");
                        
                    return Some(unescaped);
                }
            }
            
            None
        }

        pub async fn fetch_message_history(thread_id: &str, pool: &DbPool) -> Result<Vec<Message>, Error> {
            use diesel::prelude::*;
            use crate::schema::messages;
            use crate::models::conversations::Message;
            
            let mut conn = pool
                .get()
                .await
                .map_err(|e| Error::msg(format!("Failed to get database connection: {e:?}")))?;
            
            let messages = diesel_async::RunQueryDsl::load::<Message>(
                messages::table.filter(messages::thread_id.eq(thread_id)),
                &mut conn
            )
            .await
            .map_err(|e| Error::msg(format!("Failed to fetch messages: {e:?}")))?;
            
            Ok(messages)
        }

        #[cfg(feature = "ssr")]
        pub async fn send_message_stream_with_project_cancellable(
            pool: &DbPool,
            thread_id: String,
            model: String,
            active_lab: String,
            tx: mpsc::Sender<Result<Event, Infallible>>,
            cancel_token: CancellationToken,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            use log::{info, error};
            use crate::schema::threads;
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
        
            // Check for cancellation before starting
            if cancel_token.is_cancelled() {
                info!("Message stream cancelled before starting");
                return Ok(());
            }
        
            let decoded_thread_id = urlencoding::decode(&thread_id).expect("Failed to decode thread_id");
            let decoded_model = urlencoding::decode(&model).expect("Failed to decode model");
            let decoded_lab = urlencoding::decode(&active_lab).expect("failed to decode lab");
        
            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    error!("Failed to get database connection: {e}");
                    return Err(e.into());
                }
            };
        
            // Check for cancellation before database operations
            if cancel_token.is_cancelled() {
                info!("Message stream cancelled during database connection");
                return Ok(());
            }
        
            // Check if this thread is associated with a project
            let thread_info: Result<Option<uuid::Uuid>, diesel::result::Error> = threads::table
                .select(threads::project_id)
                .filter(threads::id.eq(decoded_thread_id.as_ref()))
                .first(&mut conn)
                .await;
        
            log::debug!("found thread in project: {thread_info:?}");
        
            let project_id = match thread_info {
                Ok(project_id) => project_id,
                Err(e) => {
                    error!("Failed to get thread info: {e}");
                    None
                }
            };
        
            // Check for cancellation before processing
            if cancel_token.is_cancelled() {
                info!("Message stream cancelled before processing");
                return Ok(());
            }
        
            // If this is a project thread, use the RAG service
            if let Some(proj_id) = project_id {
                info!("Processing project thread with RAG service");
                
                // Get the last user message to use as the query
                let last_user_message = fetch_message_history(&decoded_thread_id, pool)
                    .await
                    .ok()
                    .and_then(|messages| {
                        messages.into_iter()
                            .filter(|msg| msg.role == "user")
                            .next_back()
                            .and_then(|msg| msg.content)
                    });
        
                if let Some(user_query) = last_user_message {
                    // Check for cancellation before RAG processing
                    if cancel_token.is_cancelled() {
                        info!("Message stream cancelled before RAG processing");
                        return Ok(());
                    }
        
                    // Create the appropriate RAG service
                    let rag_service = match create_rag_service(&decoded_lab, decoded_model.into_owned()) {
                        Ok(service) => service,
                        Err(e) => {
                            error!("Failed to create RAG service: {e}");
                            return Err(e);
                        }
                    };
        
                    // Process the query with RAG
                    return rag_service.process_project_query(
                        pool,
                        proj_id,
                        user_query,
                        &decoded_thread_id,
                        tx,
                        cancel_token,
                    ).await;
                } else {
                    error!("No user message found for project thread");
                    // Fall back to regular chat
                }
            }
        
            // Regular chat without project context (existing logic)
            match decoded_lab.as_ref() {
                "anthropic" => {
                    let anthropic_service = AnthropicService::new(decoded_model.into_owned());
                    anthropic_service.send_message_cancellable(
                        pool, 
                        &decoded_thread_id, 
                        tx.clone(),
                        cancel_token.clone()
                    ).await
                },
                "openai" => {
                    let openai_service = OpenAIService::new(decoded_model.into_owned());
                    openai_service.send_message_cancellable(
                        pool, 
                        &decoded_thread_id, 
                        tx.clone(),
                        cancel_token.clone()
                    ).await
                },
                _ => Err(anyhow::anyhow!("unsupported lab: {}", decoded_lab)),
            }.map_err(|e| e.into())
        }
    }
}

#[component]
pub fn Chat(
    thread_id: ReadSignal<String>,
    #[prop(optional)] on_message_created: Option<Callback<()>>,
    #[prop(optional)] pending_messages: Option<WriteSignal<Vec<PendingMessage>>>,
    #[prop(optional)] on_thread_created: Option<Callback<String>>,
) -> impl IntoView {
    let (message, set_message) = signal(String::new());
    let (is_sending, set_is_sending) = signal(false);
    let (current_stream_id, set_current_stream_id) = signal::<Option<String>>(None);
    
    let (model, set_model) = signal("gpt-4o-mini".to_string());
    let (lab, set_lab) = signal("openai".to_string());

    let (toast_visible, set_toast_visible) = signal(false);
    let (toast_message, set_toast_message) = signal(String::new());

    let show_toast = move |msg: String| {
        set_toast_message(msg);
        set_toast_visible(true);
        set_timeout(
            move || set_toast_visible(false),
            std::time::Duration::from_secs(5)
        );
    };

    let handle_model_change = move |ev| {
        let value = event_target_value(&ev);
        set_model(value.clone());
    
        let new_lab = if value.contains("claude") {
            "anthropic"
        } else {
            "openai"
        };
        set_lab(new_lab.to_string());
    };

    let send_message = move || {
        let message_value = message.get();
        let current_thread_id = thread_id.get_untracked();
        let selected_model = model.get_untracked();
        let active_lab = lab.get_untracked();
    
        spawn_local(async move {
            set_is_sending(true);
    
            let user_id = match get_current_user().await {
                Ok(Some(user)) => Some(user.id),
                _ => None,
            };
    
            let is_placeholder_thread = if let Ok(_uuid) = uuid::Uuid::parse_str(&current_thread_id) {
                match check_thread_exists(current_thread_id.clone()).await {
                    Ok(exists) => !exists,
                    Err(_) => true,
                } 
            } else { 
                false
            };
    
            // 1. Save user message to DB immediately
            let user_message_view = NewMessageView {
                thread_id: current_thread_id.clone(),
                content: Some(message_value.clone()),
                role: "user".to_string(),
                active_model: selected_model.clone(),
                active_lab: active_lab.clone(),
                user_id,
            };
    
            match create_message(user_message_view, false).await {
                Ok(_) => {
                    set_message.set(String::new());
    
                    if is_placeholder_thread {
                        if let Some(callback) = on_thread_created {
                            callback.run(current_thread_id.clone());
                        }
                    }
                    
                    if let Some(callback) = on_message_created {
                        callback.run(());
                    }
    
                    // 2. Create pending assistant message
                    let pending_id = uuid::Uuid::new_v4().to_string();
                    let pending_msg = PendingMessage {
                        id: pending_id.clone(),
                        thread_id: current_thread_id.clone(),
                        content: String::new(),
                        role: "assistant".to_string(),
                        active_model: selected_model.clone(),
                        active_lab: active_lab.clone(),
                        is_streaming: true,
                        created_at: Utc::now(),
                    };
                    
                    // 3. Add to pending messages if available
                    if let Some(set_pending) = pending_messages {
                        set_pending.update(|msgs| msgs.push(pending_msg));
                    }
    
                    // 4. First create a stream
                    let window = web_sys::window().unwrap();
                    let resp_value = match JsFuture::from(window.fetch_with_str("/api/create-stream")).await {
                        Ok(val) => val,
                        Err(e) => {
                            error!("Failed to create stream: {e:?}");
                            set_is_sending(false);
                            // Remove pending message on error
                            if let Some(set_pending) = pending_messages {
                                set_pending.update(|msgs| {
                                    msgs.retain(|m| m.id != pending_id)
                                });
                            }
                            show_toast("Failed to create message stream. Please try again.".to_string());
                            return;
                        }
                    };
    
                    let resp = resp_value.dyn_into::<web_sys::Response>().unwrap();
                    let json = match JsFuture::from(resp.json().unwrap()).await {
                        Ok(json) => json,
                        Err(e) => {
                            error!("Failed to parse stream response: {e:?}");
                            set_is_sending(false);
                            // Remove pending message on error
                            if let Some(set_pending) = pending_messages {
                                set_pending.update(|msgs| {
                                    msgs.retain(|m| m.id != pending_id)
                                });
                            }
                            show_toast("Failed to create message stream. Please try again.".to_string());
                            return;
                        }
                    };
    
                    let stream_data: StreamResponse = match serde_wasm_bindgen::from_value(json) {
                        Ok(data) => data,
                        Err(e) => {
                            error!("Failed to deserialize stream response: {e:?}");
                            set_is_sending(false);
                            // Remove pending message on error
                            if let Some(set_pending) = pending_messages {
                                set_pending.update(|msgs| {
                                    msgs.retain(|m| m.id != pending_id)
                                });
                            }
                            show_toast("Failed to create message stream. Please try again.".to_string());
                            return;
                        }
                    };
    
                    let stream_id = stream_data.stream_id;
                    set_current_stream_id(Some(stream_id.clone()));
    
                    // 5. Set up SSE stream to collect content using the stream_id
                    let mut accumulated_content = String::new();
                    let mut current_citations: Vec<DocumentCitation> = Vec::new();
                    
                    let thread_id_value = thread_id.get_untracked().to_string();
                    let active_model_value = model.get_untracked().to_string();
                    let active_lab_value = lab.get_untracked().to_string();
    
                    let url = format!(
                        "/api/send_message_stream?stream_id={}&thread_id={}&model={}&lab={}",
                        urlencoding::encode(&stream_id),
                        urlencoding::encode(&thread_id_value),
                        urlencoding::encode(&active_model_value),
                        urlencoding::encode(&active_lab_value)
                    );
    
                    let event_source = EventSource::new(&url)
                        .expect("Failed to connect to SSE endpoint");
                    
                    let event_source_clone = event_source.clone();
                    let on_message = {
                        let on_message_created_clone = on_message_created;
                        let pending_id_clone = pending_id.clone();
                        Closure::wrap(Box::new(move |event: MessageEvent| {
                            if let Some(data) = event.data().as_string() {
                                // Handle simple text responses (non-RAG)
                                if data == "[DONE]" {
                                    event_source_clone.close();
                                    set_is_sending(false);
                                    set_current_stream_id(None);
    
                                    // 6. Save final content to DB and remove from pending
                                    let final_content = accumulated_content.clone();
                                    let is_llm = true;
                                    let assistant_message_view = NewMessageView {
                                        thread_id: thread_id.get_untracked().clone(),
                                        content: Some(final_content),
                                        role: "assistant".to_string(),
                                        active_model: model.get_untracked().clone(),
                                        active_lab: lab.get_untracked().clone(),
                                        user_id,
                                    };
    
                                    spawn_local(async move {
                                        if let Err(e) = create_message(assistant_message_view, is_llm).await {
                                            error!("Failed to create LLM message: {e:?}");
                                        } 
                                        if let Some(callback) = on_message_created_clone {
                                            callback.run(());
                                        }
                                    });
    
                                    // Remove from pending messages
                                    if let Some(set_pending) = pending_messages {
                                        set_pending.update(|msgs| {
                                            msgs.retain(|m| m.id != pending_id_clone)
                                        });
                                    }
                                    return;
                                } else if data == "[CANCELLED]" {
                                    event_source_clone.close();
                                    set_is_sending(false);
                                    set_current_stream_id(None);
                                    
                                    // Remove from pending messages
                                    if let Some(set_pending) = pending_messages {
                                        set_pending.update(|msgs| {
                                            msgs.retain(|m| m.id != pending_id_clone)
                                        });
                                    }
                                    return;
                                }
    
                                // Try to parse as RAG response first
                                match serde_json::from_str::<RagResponse>(&data) {
                                    Ok(rag_response) => {
                                        match rag_response.message_type.as_str() {
                                            "status" => {
                                                // Update pending message with status
                                                if let Some(status) = rag_response.status {
                                                    if let Some(set_pending) = pending_messages {
                                                        set_pending.update(|msgs| {
                                                            if let Some(msg) = msgs.iter_mut().find(|m| m.id == pending_id_clone) {
                                                                msg.content = format!("🔍 {}", status);
                                                            }
                                                        });
                                                    }
                                                }
                                            }
                                            "citations" => {
                                                if let Some(citations) = rag_response.citations {
                                                    current_citations = citations;
                                                    // Optionally update the pending message to show citations received
                                                    if let Some(set_pending) = pending_messages {
                                                        set_pending.update(|msgs| {
                                                            if let Some(msg) = msgs.iter_mut().find(|m| m.id == pending_id_clone) {
                                                                msg.content = format!("📄 Found {} relevant documents...", current_citations.len());
                                                            }
                                                        });
                                                    }
                                                }
                                            }
                                            "content" => {
                                                if let Some(content) = rag_response.content {
                                                    accumulated_content.push_str(&content);
                                                    
                                                    // Update pending message content
                                                    if let Some(set_pending) = pending_messages {
                                                        set_pending.update(|msgs| {
                                                            if let Some(msg) = msgs.iter_mut().find(|m| m.id == pending_id_clone) {
                                                                msg.content = accumulated_content.clone();
                                                            }
                                                        });
                                                    }
                                                }
                                            }
                                            "error" => {
                                                if let Some(error_content) = rag_response.content {
                                                    error!("RAG Error: {}", error_content);
                                                    
                                                    // Update pending message with error
                                                    if let Some(set_pending) = pending_messages {
                                                        set_pending.update(|msgs| {
                                                            if let Some(msg) = msgs.iter_mut().find(|m| m.id == pending_id_clone) {
                                                                msg.content = format!("❌ Error: {}", error_content);
                                                                msg.is_streaming = false;
                                                            }
                                                        });
                                                    }
                                                }
                                                
                                                event_source_clone.close();
                                                set_is_sending(false);
                                                set_current_stream_id(None);
                                            }
                                            "done" => {
                                                event_source_clone.close();
                                                set_is_sending(false);
                                                set_current_stream_id(None);
    
                                                // Create final content with citations if available
                                                let mut final_content = accumulated_content.clone();
                                                if !current_citations.is_empty() {
                                                    final_content.push_str("\n\n**Sources:**\n");
                                                    for citation in &current_citations {
                                                        final_content.push_str(&format!(
                                                            "- **{}** (similarity: {:.2})\n",
                                                            citation.filename,
                                                            citation.similarity
                                                        ));
                                                    }
                                                }
    
                                                // Save final content to DB and remove from pending
                                                let is_llm = true;
                                                let assistant_message_view = NewMessageView {
                                                    thread_id: thread_id.get_untracked().clone(),
                                                    content: Some(final_content),
                                                    role: "assistant".to_string(),
                                                    active_model: model.get_untracked().clone(),
                                                    active_lab: lab.get_untracked().clone(),
                                                    user_id,
                                                };
    
                                                spawn_local(async move {
                                                    if let Err(e) = create_message(assistant_message_view, is_llm).await {
                                                        error!("Failed to create LLM message: {e:?}");
                                                    } 
                                                    if let Some(callback) = on_message_created_clone {
                                                        callback.run(());
                                                    }
                                                });
    
                                                // Remove from pending messages
                                                if let Some(set_pending) = pending_messages {
                                                    set_pending.update(|msgs| {
                                                        msgs.retain(|m| m.id != pending_id_clone)
                                                    });
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    Err(_) => {
                                        // Not a RAG response, handle as regular streaming text
                                        accumulated_content.push_str(&data);
                                        
                                        // Update pending message content
                                        if let Some(set_pending) = pending_messages {
                                            set_pending.update(|msgs| {
                                                if let Some(msg) = msgs.iter_mut().find(|m| m.id == pending_id_clone) {
                                                    msg.content = accumulated_content.clone();
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                        }) as Box<dyn FnMut(_)>)
                    };
    
                    let event_source_error = event_source.clone();
                    let on_error = {
                        let pending_id_clone = pending_id.clone();
                        Closure::wrap(Box::new(move |error: ErrorEvent| {
                            error!("SSE Error: {error:?}");
                            if let Some(es) = error.target()
                                .and_then(|t| t.dyn_into::<web_sys::EventSource>().ok())
                            {
                                if es.ready_state() == web_sys::EventSource::CLOSED {
                                    // Handle connection closed
                                    info!("EventSource connection closed");
                                }
                            }
                            event_source_error.close();
                            set_is_sending(false);
                            set_current_stream_id(None);
                            
                            // Remove from pending messages on error
                            if let Some(set_pending) = pending_messages {
                                set_pending.update(|msgs| {
                                    msgs.retain(|m| m.id != pending_id_clone)
                                });
                            }
                        }) as Box<dyn FnMut(_)>)
                    };
    
                    event_source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
                    event_source.set_onerror(Some(on_error.as_ref().unchecked_ref()));
                    on_message.forget();
                    on_error.forget();
                }
                Err(e) => {
                    error!("Failed to create message: {e:?}");
                    let error_msg = e.to_string();
                    if error_msg.contains("Daily message limit") {
                        show_toast("You've reached your daily limit of 40 messages. Try again tomorrow!".to_string());
                    } else {
                        show_toast("Failed to send message. Please try again.".to_string());
                    }
    
                    set_is_sending(false);
                }
            }
        });
    };

    // Add cancel function
    let cancel_message = move || {
        if let Some(stream_id) = current_stream_id.get() {
            let window = web_sys::window().unwrap();
            let url = format!("/api/cancel-stream?stream_id={}", stream_id);

            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(_) = JsFuture::from(window.fetch_with_str(&url)).await {
                    info!("Stream cancelled");
                }
            });
            
            set_is_sending(false);
            set_current_stream_id(None);
            
            // Remove pending message on cancellation
            if let Some(set_pending) = pending_messages {
                set_pending.update(|msgs| {
                    // Remove the last pending message (the one being cancelled)
                    if let Some(last_idx) = msgs.iter().rposition(|m| m.is_streaming) {
                        msgs.remove(last_idx);
                    }
                });
            }
        }
    };

    let send_message_action = move |_: web_sys::MouseEvent| {
        if is_sending.get() {
            cancel_message();
        } else {
            send_message();
        }
    };

    view! {
        <div class="relative">
            <div class="flex flex-col space-y-3 pl-3 pr-3 bg-gray-300 dark:bg-teal-900 border-gray-300 dark:border-teal-600">
                <div class="flex space-x-3">
                    <textarea
                        class="flex-1 pt-3 pl-3 rounded-lg resize-none min-h-[2.5rem] max-h-32
                        text-gray-800 dark:text-gray-200 
                        bg-gray-100 dark:bg-teal-700 
                        border border-gray-400 dark:border-teal-600
                        focus:border-seafoam-500 dark:focus:border-mint-400 focus:outline-none focus:ring-2 focus:ring-seafoam-500/20 dark:focus:ring-mint-400/20
                        placeholder-gray-500 dark:placeholder-gray-400
                        transition duration-0 ease-in-out"
                        placeholder="Type your message..."
                        prop:value=message
                        on:input=move |event| {
                            set_message(event_target_value(&event));
                            let target = event.target().unwrap();
                            let style = target.unchecked_ref::<HtmlElement>().style();
                            style.set_property("height", "auto").unwrap();
                            style
                                .set_property(
                                    "height",
                                    &format!(
                                        "{}px",
                                        target.unchecked_ref::<HtmlElement>().scroll_height(),
                                    ),
                                )
                                .unwrap();
                        }
                        on:keydown=move |event| {
                            if event.key() == "Enter" && !event.shift_key() {
                                event.prevent_default();
                                if !message.get().trim().is_empty() && !is_sending.get() {
                                    send_message();
                                }
                            }
                        }
                    >
                    </textarea>
                    <button
                        class="ib px-6 rounded-lg font-medium
                        text-white transition duration-200 ease-in-out
                        disabled:cursor-not-allowed disabled:opacity-50
                        focus:outline-none focus:ring-2 focus:ring-offset-2"
                        class:bg-seafoam-600=move || !is_sending.get()
                        class:hover:bg-seafoam-700=move || !is_sending.get()
                        class:focus:ring-seafoam-500=move || !is_sending.get()
                        class:dark:bg-teal-600=move || !is_sending.get()
                        class:dark:hover:bg-teal-700=move || !is_sending.get()
                        class:bg-seafoam-400=move || is_sending.get()
                        class:hover:bg-salmon-600=move || is_sending.get()
                        class:dark:bg-seafoam-500=move || is_sending.get()
                        class:dark:hover:bg-salmon-600=move || is_sending.get()
                        on:click=send_message_action
                        disabled=move || (!is_sending.get() && message.get().trim().is_empty())
                    >
                        {move || if is_sending.get() { "cancel" } else { "yap" }}
                    </button>
                </div>

                // Bottom row with model selector on left and instructions centered
                <div class="flex items-center justify-between">
                    <select
                        class="text-xs px-3 py-2 rounded-md
                        text-gray-700 dark:text-gray-300 
                        bg-gray-100 dark:bg-teal-700 
                        border border-gray-400 dark:border-teal-600
                        hover:border-gray-600 dark:hover:border-teal-400
                        focus:border-seafoam-500 dark:focus:border-mint-400 focus:outline-none
                        transition duration-200 ease-in-out"
                        on:change=handle_model_change
                        prop:value=move || model.get()
                    >
                        <option value="claude-3-haiku-20240307">"claude-3-haiku"</option>
                        <option value="claude-3-sonnet-20240229">"claude-3-sonnet"</option>
                        <option value="claude-3-opus-20240229">"claude-3-opus"</option>
                        <option value="claude-3-5-sonnet-20240620">"claude-3-5-sonnet"</option>
                        <option value="gpt-4o-mini">"gpt-4o-mini"</option>
                        <option value="gpt-4o">"gpt-4o"</option>
                        <option value="gpt-4-turbo">"gpt-4-turbo"</option>
                    </select>

                    <div class="flex-1 text-center">
                        <div class="text-xs text-gray-500 dark:text-gray-400">
                            "Press Enter to send • Shift+Enter for new line"
                        </div>
                    </div>

                    <div class="w-[120px]"></div>
                </div>
            </div>
            <Toast
                message=toast_message
                visible=toast_visible
                on_close=move || set_toast_visible(false)
            />
        </div>
    }.into_any()
}

#[server(CreateMessage, "/api")]
pub async fn create_message(new_message_view: NewMessageView, is_llm: bool) -> Result<(), ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::{AsyncPgConnection, AsyncConnection, RunQueryDsl};
    use diesel::sql_types::Integer;
    use std::fmt;

    use crate::state::AppState;
    use crate::models::conversations::{NewMessage, Thread};
    use crate::schema::{messages, threads};
    use crate::auth::get_current_user;

    #[derive(QueryableByName)]
    struct MessageCount {
        #[diesel(sql_type = Integer)]
        message_count: i32,
    }

    #[derive(Debug)]
    enum CreateMessageError {
        PoolError(String),
        DatabaseError(diesel::result::Error),
        Unauthorized,
        RateLimitExceeded,
    }

    impl fmt::Display for CreateMessageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                CreateMessageError::PoolError(e) => write!(f, "Pool error: {e}"),
                CreateMessageError::DatabaseError(e) => write!(f, "Database error: {e}"),
                CreateMessageError::Unauthorized => write!(f, "unauthorized - user not logged in"),
                CreateMessageError::RateLimitExceeded => write!(f, "Daily message limit of 20 reached. Try again tomorrow!"),
            }
        }
    }

    impl From<CreateMessageError> for ServerFnError {
        fn from(error: CreateMessageError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| CreateMessageError::PoolError(e.to_string()))?;

    let current_user = get_current_user().await.map_err(|_| CreateMessageError::Unauthorized)?;
    let user_id = current_user.ok_or(CreateMessageError::Unauthorized)?.id;

    let new_message: NewMessage = new_message_view.clone().into();

    // check if the user hit rate limit
    async fn check_increment_rate_limit(user_id: i32, conn: &mut AsyncPgConnection) -> Result<bool, diesel::result::Error> {
        use diesel_async::RunQueryDsl;

        // upsert daily counter
        let query = "INSERT INTO daily_usage (user_id, usage_date, message_count)
            VALUES ($1, CURRENT_DATE, 1)
            ON CONFLICT (user_id, usage_date)
            DO UPDATE SET
              message_count = daily_usage.message_count + 1,
              updated_at = CURRENT_TIMESTAMP
            RETURNING message_count";

        let result: MessageCount = diesel::sql_query(query)
            .bind::<diesel::sql_types::Integer, _>(user_id)
            .get_result(conn)
            .await?;

        Ok(result.message_count <= 40)
    }

    if !is_llm {
        let rate_limit_ok = check_increment_rate_limit(user_id, &mut conn)
            .await
            .map_err(CreateMessageError::DatabaseError)?;
        
        if !rate_limit_ok {
            return Err(CreateMessageError::RateLimitExceeded.into());
        }
    }

    // Use async transaction
    let is_first_user_message = conn.transaction(|conn| {
        Box::pin(async move {
            let mut thread_was_created = false;
            
            if !is_llm {
                let thread_id = &new_message.thread_id;
                
                // Check if thread exists - explicitly use async version
                let thread_exists = diesel_async::RunQueryDsl::first::<Thread>(
                    threads::table.find(thread_id),
                    conn
                )
                .await
                .optional()?
                .is_some();

                if !thread_exists {
                    let new_thread = Thread {
                        id: thread_id.clone(),
                        created_at: None,
                        updated_at: None,
                        user_id: Some(user_id),
                        parent_thread_id: None,
                        branch_point_message_id: None,
                        branch_name: None,
                        title: None,
                        project_id: None,
                    };
                    
                    diesel_async::RunQueryDsl::execute(
                        diesel::insert_into(threads::table).values(&new_thread),
                        conn
                    )
                    .await?;
                    
                    thread_was_created = true;
                }
            }

            diesel_async::RunQueryDsl::execute(
                diesel::insert_into(messages::table).values(&new_message),
                conn
            )
            .await?;

            if !is_llm {
                log::debug!("Message successfully inserted into the database: {new_message:?}");
            }

            // Calculate is_first_user_message AFTER thread creation and message insertion
            let is_first_message = if !is_llm && new_message.role == "user" {
                if thread_was_created {
                    // If we just created the thread, this is definitely the first user message
                    true
                } else {
                    // Thread already existed - check if this is the first user message
                    let current_thread_result: Result<Thread, diesel::result::Error> = threads::table
                        .filter(threads::id.eq(&new_message.thread_id))
                        .get_result(conn)
                        .await;
                    
                    match current_thread_result {
                        Ok(thread) => {
                            if let Some(_parent_thread_id) = thread.parent_thread_id {
                                // This is a branch - check if any NEW messages have been added since creation
                                let messages_added_after_branch: i64 = messages::table
                                    .filter(messages::thread_id.eq(&new_message.thread_id))
                                    .filter(messages::created_at.gt(thread.created_at.unwrap_or_default()))
                                    .filter(messages::role.eq("user"))
                                    .count()
                                    .get_result(conn)
                                    .await?;
                                messages_added_after_branch == 1 // Should be 1 because we just inserted this message
                            } else {
                                // Root thread - check total user message count
                                let message_count: i64 = messages::table
                                    .filter(messages::thread_id.eq(&new_message.thread_id))
                                    .filter(messages::role.eq("user"))
                                    .count()
                                    .get_result(conn)
                                    .await?;
                                message_count == 1 // Should be 1 because we just inserted this message
                            }
                        }
                        Err(_) => false, // If we can't find the thread, something went wrong
                    }
                }
            } else {
                false
            };

            Ok(is_first_message)
        })
    })
    .await
    .map_err(CreateMessageError::DatabaseError)?;

    if is_first_user_message {
        if let Some(content) = new_message_view.content {
            let app_state_clone = app_state.clone();
            let thread_id = new_message_view.thread_id.clone();
            let content_clone = content.clone();

            tokio::spawn(async move {
                #[cfg(feature = "ssr")]
                {
                    crate::services::title_generation::generate_and_update_title_with_sse(
                        app_state_clone,
                        user_id,
                        thread_id,
                        content_clone
                    ).await;
                }
            });
        }
    }

    Ok(())
}

#[server(CheckThreadExists, "/api")]
pub async fn check_thread_exists(thread_id: String) -> Result<bool, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::schema::threads;
    use crate::models::conversations::Thread;
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum CheckThreadExistsError {
        PoolError(String),
        DatabaseError(diesel::result::Error),
        Unauthorized,
    }

    impl fmt::Display for CheckThreadExistsError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                CheckThreadExistsError::PoolError(e) => write!(f, "Pool error: {e}"),
                CheckThreadExistsError::DatabaseError(e) => write!(f, "Database error: {e}"),
                CheckThreadExistsError::Unauthorized => write!(f, "unauthorized - user not logged in"),
            }
        }
    }

    impl From<CheckThreadExistsError> for ServerFnError {
        fn from(error: CheckThreadExistsError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    impl From<diesel::result::Error> for CheckThreadExistsError {
        fn from(error: diesel::result::Error) -> Self {
            CheckThreadExistsError::DatabaseError(error)
        }
    }

    let current_user = get_current_user().await.map_err(|_| CheckThreadExistsError::Unauthorized)?;
    let _user_id = current_user.ok_or(CheckThreadExistsError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| CheckThreadExistsError::PoolError(e.to_string()))?;

    let thread_exists = threads::table
        .find(&thread_id)
        .first::<Thread>(&mut conn)
        .await
        .optional()
        .map_err(CheckThreadExistsError::DatabaseError)?
        .is_some();

    Ok(thread_exists)
}
