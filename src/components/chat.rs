use cfg_if::cfg_if;
use leptos::{prelude::*, task::spawn_local};
use log::error;
use urlencoding;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, MessageEvent, ErrorEvent, HtmlElement};
use chrono::Utc;

use crate::{auth::get_current_user, models::conversations::{NewMessageView, PendingMessage}};

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::response::sse::Event;
        use anyhow::{anyhow, Error};
        use reqwest::Client;
        use regex::Regex;
        use std::env;
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use tokio::sync::mpsc;
        use futures::stream::{Stream, StreamExt};
        use log::debug;

        use crate::database::db::DbPool;
        use crate::models::conversations::Message;

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

                let re = Regex::new(r#""text":"([^"]*)""#).unwrap();
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec()).map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            debug!("Trimmed event: {}", event.trim());

                            for line in event.trim().lines() {
                                if line.trim() == "event: message_stop" {
                                    debug!("Received message_stop event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    break;
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    for cap in re.captures_iter(json_str) {
                                        let content = cap[1].to_string();
                                        debug!("Extracted content: {content}");
                                        tx.send(Ok(Event::default().data(content))).await.ok();
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

        impl OpenAIService {
            pub fn new(model: String) -> Self {
                let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set.");
                let client = Client::new();
                OpenAIService { client, api_key, model }
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

                let re = Regex::new(r#""content":"([^"]*)""#).unwrap();
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(bytes) => {
                            let event = String::from_utf8(bytes.to_vec()).map_err(|e| anyhow!("Failed to convert bytes to string: {}", e))?;
                            debug!("Trimmed event: {}", event.trim());

                            for line in event.trim().lines() {
                                if line.trim() == "data: [DONE]" {
                                    debug!("Received [DONE] event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    break;
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    for cap in re.captures_iter(json_str) {
                                        let content = cap[1].to_string();
                                        debug!("Extracted content: {content}");
                                        tx.send(Ok(Event::default().data(content))).await.ok();
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

        pub async fn send_message_stream(
            pool: &DbPool,
            thread_id: String,
            model: String,
            active_lab: String,
            tx: mpsc::Sender<Result<Event, std::convert::Infallible>>
        ) {
            let decoded_thread_id = urlencoding::decode(&thread_id).expect("Failed to decode thread_id");
            let decoded_model = urlencoding::decode(&model).expect("Failed to decode model");
            let decoded_lab = urlencoding::decode(&active_lab).expect("failed to decode lab");
        
            let result = match decoded_lab.as_ref() {
                "anthropic" => {
                    let anthropic_service = AnthropicService::new(decoded_model.into_owned());
                    anthropic_service.send_message(pool, &decoded_thread_id, tx.clone()).await
                },
                "openai" => {
                    let openai_service = OpenAIService::new(decoded_model.into_owned());
                    openai_service.send_message(pool, &decoded_thread_id, tx.clone()).await
                },
                _ => Err(anyhow::anyhow!("unsupported lab: {}", decoded_lab)),
            };
        
            if let Err(e) = result {
                error!("Error in send_message_stream: {e}");
                let error_event = Event::default().data(format!("Error: {e}"));
                let _ = tx.send(Ok(error_event)).await;
            }
        }
    }
}

#[component]
pub fn Chat(
    thread_id: ReadSignal<String>,
    #[prop(optional)] on_message_created: Option<Callback<()>>,
    #[prop(optional)] pending_messages: Option<WriteSignal<Vec<PendingMessage>>>,
) -> impl IntoView {
    let (message, set_message) = signal(String::new());
    let (is_sending, set_is_sending) = signal(false);
    
    let (model, set_model) = signal("gpt-4o-mini".to_string());
    let (lab, set_lab) = signal("openai".to_string());

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

                    // 4. Set up SSE stream to collect content
                    let mut accumulated_content = String::new();
                    
                    let thread_id_value = thread_id.get_untracked().to_string();
                    let active_model_value = model.get_untracked().to_string();
                    let active_lab_value = lab.get_untracked().to_string();
                    let event_source = Rc::new(EventSource::new(
                            &format!("/api/send_message_stream?thread_id={}&model={}&lab={}",
                            urlencoding::encode(&thread_id_value),
                            urlencoding::encode(&active_model_value),
                            urlencoding::encode(&active_lab_value))
                        ).expect("Failed to connect to SSE endpoint"));
        
        			let on_message = {
        				let event_source = Rc::clone(&event_source);
                        let on_message_created_clone = on_message_created;
                        let pending_id_clone = pending_id.clone();
        				Closure::wrap(Box::new(move |event: MessageEvent| {
        					let data = event.data().as_string().unwrap();
        					if data == "[DONE]" {
                                // 5. Save final content to DB and remove from pending
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
                                    } else {
                                        if let Some(callback) = on_message_created_clone {
                                            callback.run(());
                                        }
                                    }
                                });

                                // Remove from pending messages
                                if let Some(set_pending) = pending_messages {
                                    set_pending.update(|msgs| {
                                        msgs.retain(|m| m.id != pending_id_clone)
                                    });
                                }

        						set_is_sending.set(false);
        						event_source.close();
        					} else {
                                // 6. Stream tokens into pending message
                                let processed_data = data.replace("\\n", "\n");
                                accumulated_content.push_str(&processed_data);
                                
                                // Update pending message content
                                if let Some(set_pending) = pending_messages {
                                    set_pending.update(|msgs| {
                                        if let Some(msg) = msgs.iter_mut().find(|m| m.id == pending_id_clone) {
                                            msg.content = accumulated_content.clone();
                                        }
                                    });
                                }
        					}
        				}) as Box<dyn FnMut(_)>)
        			};
        
        			event_source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        			on_message.forget();
        
        			let on_error = {
        				let event_source = Rc::clone(&event_source);
        				Closure::wrap(Box::new(move |event: ErrorEvent| {
                            let error_message = format!(
        						"Error receiving message: type = {:?}, message = {:?}, filename = {:?}, lineno = {:?}, colno = {:?}, error = {:?}",
                                event.type_(),
                                event.message(),
                                event.filename(),
                                event.lineno(),
                                event.colno(),
                                event.error()
                            );
        					error!("{error_message}");
        					set_is_sending.set(false);
        					event_source.close();
        				}) as Box<dyn FnMut(_)>)
        			};
        
        			event_source.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        			on_error.forget();
                }
                Err(e) => {
                    error!("Failed to create message: {e:?}");
                    set_is_sending(false);
                }
            }
        });
    };

    let send_message_action = move |_: web_sys::MouseEvent| {
        send_message();
    };

    view! {
        <div class="flex flex-col space-y-4">
            <div class="flex flex-col space-y-3">
                <div class="flex justify-center">
                    <select
                        class="ib text-xs md:text-sm px-3 py-2 rounded-md
                        text-gray-900 dark:text-gray-100 
                        bg-white dark:bg-teal-700 
                        border border-gray-400 dark:border-teal-600
                        hover:border-gray-600 dark:hover:border-teal-400
                        focus:border-seafoam-500 dark:focus:border-aqua-400 focus:outline-none
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
                </div>

                <div class="flex space-x-3">
                    <textarea
                        class="ir text-sm flex-1 p-3 rounded-lg resize-none min-h-[2.5rem] max-h-32
                        text-gray-800 dark:text-gray-200 
                        bg-white dark:bg-teal-700 
                        border border-gray-400 dark:border-teal-600
                        focus:border-seafoam-500 dark:focus:border-aqua-400 focus:outline-none focus:ring-2 focus:ring-seafoam-500/20
                        placeholder-gray-500 dark:placeholder-gray-400
                        transition duration-200 ease-in-out"
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
                        class="ib px-6 py-3 rounded-lg font-medium
                        text-white transition duration-200 ease-in-out
                        disabled:cursor-not-allowed disabled:opacity-50
                        focus:outline-none focus:ring-2 focus:ring-offset-2"
                        class:bg-seafoam-600=move || !is_sending.get()
                        class:hover:bg-seafoam-700=move || !is_sending.get()
                        class:focus:ring-seafoam-500=move || !is_sending.get()
                        class:dark:bg-teal-600=move || !is_sending.get()
                        class:dark:hover:bg-teal-700=move || !is_sending.get()
                        class:bg-gray-400=move || is_sending.get()
                        class:dark:bg-gray-600=move || is_sending.get()
                        on:click=send_message_action
                        disabled=move || is_sending.get() || message.get().trim().is_empty()
                    >
                        {move || if is_sending.get() { "yapping..." } else { "yap" }}
                    </button>
                </div>

                <div class="text-xs text-gray-500 dark:text-gray-400 text-center">
                    "Press Enter to send • Shift+Enter for new line"
                </div>
            </div>
        </div>
    }
}

#[server(CreateMessage, "/api")]
pub async fn create_message(new_message_view: NewMessageView, is_llm: bool) -> Result<(), ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::{AsyncConnection, RunQueryDsl};
    use std::fmt;

    use crate::state::AppState;
    use crate::models::conversations::{NewMessage, Thread};
    use crate::schema::{messages, threads};
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum CreateMessageError {
        PoolError(String),
        DatabaseError(diesel::result::Error),
        Unauthorized,
    }

    impl fmt::Display for CreateMessageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                CreateMessageError::PoolError(e) => write!(f, "Pool error: {e}"),
                CreateMessageError::DatabaseError(e) => write!(f, "Database error: {e}"),
                CreateMessageError::Unauthorized => write!(f, "unauthorized - user not logged in"),
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

    let is_first_user_message = if !is_llm && new_message.role == "user" {
        // Get the current thread to check if it's a branch
        let current_thread_result: Result<Thread, diesel::result::Error> = threads::table
            .filter(threads::id.eq(&new_message.thread_id))
            .get_result(&mut conn)
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
                        .get_result(&mut conn)
                        .await
                        .map_err(CreateMessageError::DatabaseError)?;
                    messages_added_after_branch == 0
                } else {
                    // Root thread - use original logic
                    let message_count: i64 = messages::table
                        .filter(messages::thread_id.eq(&new_message.thread_id))
                        .filter(messages::role.eq("user"))
                        .count()
                        .get_result(&mut conn)
                        .await
                        .map_err(CreateMessageError::DatabaseError)?;
                    message_count == 0
                }
            }
            Err(diesel::result::Error::NotFound) => {
                // Thread doesn't exist, so definitely not the first message
                false
            }
            Err(e) => {
                // Other database error
                return Err(CreateMessageError::DatabaseError(e).into());
            }
        }
    } else {
        false
    };

    // Use async transaction
    conn.transaction(|conn| {
        Box::pin(async move {
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
                    };
                    
                    diesel_async::RunQueryDsl::execute(
                        diesel::insert_into(threads::table).values(&new_thread),
                        conn
                    )
                    .await?;
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

            Ok(())
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
