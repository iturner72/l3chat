use leptos::prelude::*;
use web_sys::{window, Element};
use wasm_bindgen::JsCast;

use crate::models::conversations::MessageView;
use crate::components::threadlist::{create_branch, get_thread_branches};

#[component]
pub fn MessageList(
    current_thread_id: ReadSignal<String>,
    set_current_thread_id: WriteSignal<String>,
    #[prop(optional)] refetch_trigger: Option<ReadSignal<i32>>,
) -> impl IntoView {
    // Create a signal to track refetch triggers
    let (internal_refetch_trigger, set_internal_refetch_trigger) = signal(0);

    // Combine external and internal triggers
    let combined_trigger = move || {
        let internal = internal_refetch_trigger.get();
        let external = refetch_trigger.map(|t| t.get()).unwrap_or(0);
        (current_thread_id.get(), internal, external)
    };

    let messages_resource = Resource::new(
        combined_trigger,
        |(thread_id, _, _)| async move { 
            if thread_id.is_empty() {
                Ok(Vec::new())
            } else {
                get_messages_for_thread(thread_id).await.map_err(|e| format!("Failed to load thread: {}", e))
            }
        }
    );

    let branches_resource = Resource::new(
        combined_trigger,
        |(thread_id, _, _)| async move {
            if !thread_id.is_empty() {
                get_thread_branches(thread_id).await.unwrap_or_default()
            } else {
                Vec::new()
            }
        }
    );

    let create_branch_action = Action::new(move |(message_id,): &(i32,)| {
        let message_id = *message_id;
        let thread_id = current_thread_id.get();
        
        async move {
            match create_branch(thread_id, message_id, None).await {
                Ok(new_thread_id) => {
                    log::info!("Created branch: {}", new_thread_id);
                    set_current_thread_id.set(new_thread_id);
                    // Trigger internal refetch
                    set_internal_refetch_trigger.update(|n| *n += 1);
                    Ok(())
                }
                Err(e) => {
                    log::error!("Failed to create branch: {:?}", e);
                    Err(format!("Failed to create branch: {}", e))
                }
            }
        }
    });

    view! {
        <div class="message-list h-108 md:h-172 space-y-8 overflow-hidden hover:overflow-y-auto flex flex-col">
            // Wrap branch navigation in its own Transition
            <Transition fallback=move || {
                view! {
                    <div class="branch-navigation p-2 bg-gray-200 dark:bg-teal-700 rounded-md mb-4">
                        <div class="animate-pulse bg-gray-300 dark:bg-teal-600 h-8 rounded"></div>
                    </div>
                }
            }>
                {move || {
                    branches_resource
                        .get()
                        .map(|branches| {
                            if !branches.is_empty() {
                                view! {
                                    <div class="branch-navigation p-2 bg-gray-200 dark:bg-teal-700 rounded-md mb-4">
                                        <h4 class="text-sm font-medium text-gray-700 dark:text-gray-200 mb-2">
                                            "Available Branches:"
                                        </h4>
                                        <div class="flex flex-wrap gap-2">
                                            {branches
                                                .into_iter()
                                                .map(|branch| {
                                                    let branch_id = branch.thread_id.clone();
                                                    let is_current = current_thread_id.get() == branch_id;
                                                    view! {
                                                        <button
                                                            class=format!(
                                                                "px-2 py-1 text-xs rounded transition-colors {}",
                                                                if is_current {
                                                                    "bg-seafoam-500 text-white"
                                                                } else {
                                                                    "bg-gray-300 dark:bg-teal-600 text-gray-700 dark:text-gray-200 hover:bg-gray-400 dark:hover:bg-teal-500"
                                                                },
                                                            )

                                                            on:click=move |_| {
                                                                set_current_thread_id.set(branch_id.clone())
                                                            }
                                                        >

                                                            "🌿 "
                                                            {branch.branch_name.unwrap_or_else(|| "branch".to_string())}
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }
                        })
                        .unwrap_or_else(|| view! { <div></div> }.into_any())
                }}

            </Transition>

            <Transition fallback=move || {
                view! {
                    <div class="space-y-4">
                        <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-md"></div>
                        <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-md"></div>
                        <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-md"></div>
                    </div>
                }
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
                                                let role = message.role.clone();
                                                let role_for_button = role.clone();
                                                let role_for_branch = role.clone();
                                                let role_for_info = role.clone();
                                                view! {
                                                    <div class=format!(
                                                        "message-wrapper flex w-full {}",
                                                        if role == "assistant" {
                                                            "justify-start"
                                                        } else {
                                                            "justify-end"
                                                        },
                                                    )>
                                                        <div class="message-container flex flex-col">
                                                            <button
                                                                class=format!(
                                                                    "message-item border-0 p-2 transition duration-0 group {}",
                                                                    if role_for_button == "assistant" {
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
                                                                            role: {role_for_info}
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

                                                            {move || {
                                                                if role_for_branch == "user" {
                                                                    view! {
                                                                        <div class="branch-actions mt-2 flex justify-end gap-2">
                                                                            <button
                                                                                class="px-2 py-1 text-xs bg-blue-500 hover:bg-blue-600 text-white rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                                                                disabled=move || create_branch_action.pending().get()
                                                                                on:click=move |_| {
                                                                                    create_branch_action.dispatch((message.id,));
                                                                                }
                                                                            >

                                                                                {move || {
                                                                                    if create_branch_action.pending().get() {
                                                                                        "⏳ creating branch..."
                                                                                    } else {
                                                                                        "+ 🌿"
                                                                                    }
                                                                                }}

                                                                            </button>
                                                                        </div>
                                                                    }
                                                                        .into_any()
                                                                } else {
                                                                    view! { <div></div> }.into_any()
                                                                }
                                                            }}

                                                        </div>
                                                    </div>
                                                }
                                            }
                                        />
                                    }
                                        .into_any()
                                }
                                Err(e) => {
                                    view! {
                                        <div class="p-4 bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 rounded-md">
                                            <h3 class="font-medium mb-2">"Error loading messages"</h3>
                                            <p class="text-sm">{e}</p>
                                            <button
                                                class="mt-2 px-3 py-1 text-xs bg-red-600 hover:bg-red-700 text-white rounded transition-colors"
                                                on:click=move |_| {
                                                    set_internal_refetch_trigger.update(|n| *n += 1)
                                                }
                                            >

                                                "Retry"
                                            </button>
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                        })
                        .unwrap_or_else(|| view! { <div></div> }.into_any())
                }}

            </Transition>
        </div>
    }
}

#[server(GetMessagesForThread, "/api")]
pub async fn get_messages_for_thread(_thread_id: String) -> Result<Vec<MessageView>, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl; 
    use std::fmt;

    use crate::state::AppState;
    use crate::models::conversations::Message;
    use crate::schema::messages::dsl::*;
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum MessageError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
    }

    impl fmt::Display for MessageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                MessageError::Pool(e) => write!(f, "Pool error: {e}"),
                MessageError::Database(e) => write!(f, "Database error: {e}"),
                MessageError::Unauthorized => write!(f, "unauthorized - user not logged in"),
            }
        }
    }

    impl From<MessageError> for ServerFnError {
        fn from(error: MessageError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    fn to_server_error(e: MessageError) -> ServerFnError {
        ServerFnError::ServerError(e.to_string())
    }

    let current_user = get_current_user().await.map_err(|_| MessageError::Unauthorized)?;
    let current_user_id = current_user.ok_or(MessageError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| MessageError::Pool(e.to_string()))
        .map_err(to_server_error)?;

    let result = messages
        .filter(user_id.eq(current_user_id))
        .filter(thread_id.eq(_thread_id))
        .order(id.asc())
        .load::<Message>(&mut conn)
        .await
        .map_err(MessageError::Database)
        .map_err(to_server_error)?;

    Ok(result.into_iter().map(MessageView::from).collect())
}
