use leptos::prelude::*;
use web_sys::{window, Element};
use wasm_bindgen::JsCast;

use crate::models::conversations::MessageView;

#[component]
pub fn MessageList(
    current_thread_id: ReadSignal<String>
) -> impl IntoView {
    // Use Resource instead of spawn_local for SSR compatibility
    let messages_resource = Resource::new(
        || (), // No dependencies, loads once
        |_| async move { get_messages().await }
    );

    view! {
        <div class="message-list h-108 md:h-172 space-y-8 overflow-hidden hover:overflow-y-auto flex flex-col">
            <Suspense fallback=move || {
                view! { <p>"Loading messages..."</p> }
            }>
                {move || {
                    messages_resource
                        .get()
                        .map(|result| {
                            match result {
                                Ok(message_list) => {
                                    view! {
                                        <For
                                            each=move || {
                                                message_list
                                                    .clone()
                                                    .into_iter()
                                                    .filter(move |message: &MessageView| {
                                                        if current_thread_id.get().is_empty() {
                                                            true
                                                        } else {
                                                            message.thread_id == current_thread_id.get()
                                                        }
                                                    })
                                            }

                                            key=|message| message.id
                                            children=move |message| {
                                                view! {
                                                    <div class=format!(
                                                        "message-wrapper flex w-full {}",
                                                        if message.role == "assistant" {
                                                            "justify-start"
                                                        } else {
                                                            "justify-end"
                                                        },
                                                    )>
                                                        <button
                                                            class=format!(
                                                                "message-item border-2 p-2 transition duration-0 group {}",
                                                                if message.role == "assistant" {
                                                                    "border-none bg-opacity-0 self-start bg-gray-300 dark:bg-teal-800 hover:bg-gray-400 dark:hover:bg-teal-900"
                                                                } else {
                                                                    "border-gray-700 dark:border-teal-700 bg-gray-300 dark:bg-teal-800 self-end hover:bg-gray-400 dark:hover:bg-teal-900"
                                                                },
                                                            )
                                                            on:click=move |_| {
                                                                let document = window().unwrap().document().unwrap();
                                                                let elements = document
                                                                    .query_selector_all(".info-for-nerds")
                                                                    .unwrap();
                                                                for i in 0..elements.length() {
                                                                    if let Some(node) = elements.item(i) {
                                                                        if let Ok(element) = node.dyn_into::<Element>() {
                                                                            let _ = element.class_list().toggle("hidden");
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        >

                                                            <div class="flex flex-row items-center space-x-2">
                                                                <img
                                                                    src="openai_square_logo.webp"
                                                                    class="w-6 h-6 rounded-full"
                                                                />
                                                                <img
                                                                    src="anthropic_square_logo.webp"
                                                                    class="w-6 h-6 rounded-full"
                                                                />
                                                                <p class="message-content ir text-base text-teal-600 dark:text-mint-400 hover:text-teal-800 dark:hover:text-mint-300">
                                                                    {message.content.clone()}
                                                                </p>
                                                            </div>
                                                            <div class="info-for-nerds flex flex-row justify-between space-x-12 pt-8 hidden">
                                                                <div class="ai-info flex flex-col space-y-1">
                                                                    <p class="message-thread_id ir text-xs text-teal-800 dark:text-mint-600 hover:text-teal-600 dark:hover:text-mint-500">
                                                                        thread id: {message.thread_id.clone()}
                                                                    </p>
                                                                    <p class="message-id ir text-xs text-teal-800 dark:text-mint-600 hover:text-teal-600 dark:hover:text-mint-500">
                                                                        message id: {message.id}
                                                                    </p>
                                                                    <p class="message-created_at ir text-xs text-teal-900 dark:text-mint-700 hover:text-teal-700 dark:hover:text-mint-600">
                                                                        {message
                                                                            .created_at
                                                                            .map(|dt| dt.format("%b %d, %I:%M %p").to_string())
                                                                            .unwrap_or_default()}
                                                                    </p>
                                                                </div>
                                                                <div class="message-info flex flex-col space-y-1">
                                                                    <p class="message-role ir text-xs text-teal-600 dark:text-mint-400 hover:text-teal-800 dark:hover:text-mint-300">
                                                                        role: {message.role.clone()}
                                                                    </p>
                                                                    <p class="message-active_lab ir text-xs text-seafoam-600 dark:text-aqua-400 hover:text-seafoam-800 dark:hover:text-aqua-300">
                                                                        lab: {message.active_lab.clone()}
                                                                    </p>
                                                                    <p class="message-active_model ib text-xs text-aqua-600 dark:text-aqua-700 hover:text-aqua-800 dark:hover:text-aqua-300">
                                                                        model: {message.active_model.clone()}
                                                                    </p>
                                                                </div>
                                                            </div>
                                                        </button>
                                                    </div>
                                                }
                                            }
                                        />
                                    }
                                        .into_any()
                                }
                                Err(_e) => {
                                    view! { <div>"Error loading messages: {e}"</div> }.into_any()
                                }
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

#[server(GetMessages, "/api")]
pub async fn get_messages() -> Result<Vec<MessageView>, ServerFnError> {
    use diesel_async::RunQueryDsl; 
    use std::fmt;

    use crate::state::AppState;
    use crate::models::conversations::Message;
    use crate::schema::messages::dsl::messages as messages_table;

    #[derive(Debug)]
    enum MessageError {
        Pool(String),
        Database(diesel::result::Error),
    }

    impl fmt::Display for MessageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                MessageError::Pool(e) => write!(f, "Pool error: {e}"),
                MessageError::Database(e) => write!(f, "Database error: {e}"),
            }
        }
    }

    fn to_server_error(e: MessageError) -> ServerFnError {
        ServerFnError::ServerError(e.to_string())
    }

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    // Get a connection from the pool
    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| MessageError::Pool(e.to_string()))
        .map_err(to_server_error)?;

    // Use async diesel query - no need for .interact()
    let result = messages_table
        .load::<Message>(&mut conn)
        .await
        .map_err(MessageError::Database)
        .map_err(to_server_error)?;

    Ok(result.into_iter().map(MessageView::from).collect())
}
