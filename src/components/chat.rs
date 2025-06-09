use cfg_if::cfg_if;
use leptos::{prelude::*, task::spawn_local};
use log::error;
use urlencoding;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, MessageEvent, ErrorEvent, HtmlElement};

use crate::{auth::get_current_user, models::conversations::NewMessageView};

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
        use log::info;

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
                info!("Sending message to OpenAI API");
                info!("Current thread id: {thread_id}");

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
                            info!("Trimmed event: {}", event.trim());

                            for line in event.trim().lines() {
                                if line.trim() == "event: message_stop" {
                                    info!("Received message_stop event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    break;
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    for cap in re.captures_iter(json_str) {
                                        let content = cap[1].to_string();
                                        info!("Extracted content: {content}");
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

                info!("Stream closed");
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
                info!("Sending message to OpenAI API");
                info!("Current thread id: {thread_id}");

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
                            info!("Trimmed event: {}", event.trim());

                            for line in event.trim().lines() {
                                if line.trim() == "data: [DONE]" {
                                    info!("Received [DONE] event");
                                    tx.send(Ok(Event::default().data("[DONE]"))).await.ok();
                                    break;
                                } else if line.trim().starts_with("data: ") {
                                    let json_str = &line.trim()[6..];
                                    for cap in re.captures_iter(json_str) {
                                        let content = cap[1].to_string();
                                        info!("Extracted content: {content}");
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

                info!("Stream closed");
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
    model: ReadSignal<String>,
    lab: ReadSignal<String>
) -> impl IntoView {
    let (message, set_message) = signal(String::new());
    let (response, set_response) = signal(String::new());
    let (is_sending, set_is_sending) = signal(false);
    let (llm_content, set_llm_content) = signal(String::new());

    let send_message_action = move |_| {
        let message_value = message.get();
        let current_thread_id = thread_id.get_untracked();
        let selected_model = model.get_untracked();
        let active_lab = lab.get_untracked();
        let role = "user";

        spawn_local(async move {
            set_is_sending(true);
            set_response.set("".to_string());
            set_llm_content.set("".to_string());
            let is_llm = false;

            // Get current user to extract user_id
            let user_id = match get_current_user().await {
                Ok(Some(user)) => Some(user.id),
                _ => None,
            };

            let new_message_view = NewMessageView {
                thread_id: current_thread_id.clone(),
                content: Some(message_value.clone()),
                role: role.to_string(),
                active_model: selected_model.clone(),
                active_lab: active_lab.clone(),
                user_id,
            };

            match create_message(new_message_view, is_llm).await {
                Ok(_) => {

                    let thread_id_value = thread_id().to_string();
                    let active_model_value = model().to_string();
                    let active_lab_value = lab().to_string();
                    let event_source = Rc::new(EventSource::new(
                            &format!("/api/send_message_stream?thread_id={}&model={}&lab={}",
                            urlencoding::encode(&thread_id_value),
                            urlencoding::encode(&active_model_value),
                            urlencoding::encode(&active_lab_value))
                        ).expect("Failed to connect to SSE endpoint"));
        
        			let on_message = {
        				let event_source = Rc::clone(&event_source);
        				Closure::wrap(Box::new(move |event: MessageEvent| {
        					let data = event.data().as_string().unwrap();
        					if data == "[DONE]" {
                                let llm_content_value = llm_content.get();
                                let is_llm = true;
                                let new_message_view = NewMessageView {
                                    thread_id: thread_id().clone(),
                                    content: Some(llm_content_value),
                                    role: "assistant".to_string(),
                                    active_model: model().clone(),
                                    active_lab: lab().clone(),
                                    user_id,
                                };

                                spawn_local(async move {
                                    if let Err(e) = create_message(new_message_view, is_llm).await {
                                        error!("Failed to create LLM message: {e:?}");
                                    }
                                });

        						set_is_sending.set(false);
        						event_source.close();
        					} else {
                                let processed_data = data.replace("\\n", "\n");
        						set_response.update(|resp| {
        							resp.push_str(&processed_data);
        							resp.to_string();
        						});
                                set_llm_content.update(|content| {
                                    content.push_str(&processed_data);
                                    content.to_string();
                                });
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
        					set_response(error_message);
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

    view! {
        <div class="flex flex-col items-center justify-between pb-2 md:pb-4">
            <div class="w-10/12 md:w-7/12 h-[calc(0vh-20px)] overflow-y-auto flex flex-col-reverse pb-0 md:pb-12">
                <Suspense fallback=|| {
                    view! {
                        <p class="ir text-base text-seafoam-500 dark:text-aqua-400">"loading..."</p>
                    }
                }>
                    {move || {
                        view! {
                            <p class="ir text-teal-700 dark:text-mint-300 whitespace-pre-wrap">
                                {response.get()}
                            </p>
                        }
                    }}
                </Suspense>
            </div>
            <div class="flex flex-row justify-center space-x-4 w-6/12 md:w-7/12">
                <textarea
                    class="ir text-sm text-gray-800 dark:text-gray-200 bg-gray-100 dark:bg-teal-800 w-full h-8 md:h-12 p-2 text-wrap
                    border-2 border-teal-600 dark:border-seafoam-600 focus:border-seafoam-500 dark:focus:border-aqua-500 focus:outline-none
                    transition duration-300 ease-in-out resize-none rounded-md"
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
                ></textarea>
                <button
                    class="ib text-white bg-seafoam-600 hover:bg-seafoam-700 dark:bg-teal-600 dark:hover:bg-teal-700
                    text-xs md:text-lg w-1/6 p-2 rounded-md transition duration-300 ease-in-out
                    disabled:bg-gray-400 dark:disabled:bg-teal-900 disabled:text-gray-600 dark:disabled:text-teal-400 disabled:cursor-not-allowed"
                    on:click=send_message_action
                    disabled=move || is_sending.get()
                >
                    {move || if is_sending.get() { "yapping..." } else { "yap" }}
                </button>
            </div>
        </div>
    }
}

#[server(CreateMessage, "/api")]
pub async fn create_message(new_message_view: NewMessageView, is_llm: bool) -> Result<(), ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::AsyncConnection;
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

    let new_message: NewMessage = new_message_view.into();

    let current_user = get_current_user().await.map_err(|_| CreateMessageError::Unauthorized)?;
    let user_id = current_user.ok_or(CreateMessageError::Unauthorized)?.id;

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
                log::info!("Message successfully inserted into the database: {new_message:?}");
            }

            Ok(())
        })
    })
    .await
    .map_err(CreateMessageError::DatabaseError)?;

    Ok(())
}
