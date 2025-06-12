use leptos::prelude::*;
use chrono::Utc;

use crate::models::conversations::{MessageView, DisplayMessage, PendingMessage};
use crate::components::threadlist::{create_branch, get_thread_branches};
use crate::components::markdown::MarkdownRenderer;

#[component]
pub fn MessageList(
    current_thread_id: ReadSignal<String>,
    set_current_thread_id: WriteSignal<String>,
    #[prop(optional)] refetch_trigger: Option<ReadSignal<i32>>,
    #[prop(optional)] pending_messages: Option<ReadSignal<Vec<PendingMessage>>>,
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

    let combined_messages = move || -> Vec<DisplayMessage> {
        let db_messages = messages_resource.try_get()
            .and_then(|result| result?.ok())
            .unwrap_or_default();
        
        let pending = pending_messages
            .map(|p| p.get())
            .unwrap_or_default();
        
        let current_thread = current_thread_id.get();
        
        let mut combined: Vec<DisplayMessage> = Vec::new();
        
        for msg in db_messages {
            if msg.thread_id == current_thread {
                combined.push(DisplayMessage::Persisted(msg));
            }
        }
        
        for msg in pending {
            if msg.thread_id == current_thread {
                combined.push(DisplayMessage::Pending(msg));
            }
        }
        
        combined.sort_by(|a, b| {
            let a_time = a.created_at().unwrap_or_else(|| Utc::now());
            let b_time = b.created_at().unwrap_or_else(|| Utc::now());
            a_time.cmp(&b_time)
        });
        
        combined
    };

    let create_branch_action = Action::new(move |(message_id,): &(i32,)| {
        let message_id = *message_id;
        let thread_id = current_thread_id.get();
        
        async move {
            match create_branch(thread_id, message_id, None).await {
                Ok(new_thread_id) => {
                    log::info!("Created branch: {}", new_thread_id);
                    set_current_thread_id.set(new_thread_id);
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
        <div class="h-full flex flex-col w-full overflow-hidden">
            <div class="flex-shrink-0 mb-4">
                <Transition fallback=move || {
                    view! {
                        <div class="animate-pulse bg-gray-300 dark:bg-teal-600 h-8 rounded-md"></div>
                    }
                }>
                    {move || {
                        branches_resource
                            .get()
                            .map(|branches| {
                                if !branches.is_empty() {
                                    view! {
                                        <div class="p-3 bg-gray-200 dark:bg-teal-700 rounded-lg border border-gray-300 dark:border-teal-600">
                                            <h4 class="text-sm font-medium text-gray-700 dark:text-gray-200 mb-3">
                                                "Thread Branches:"
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
                                                                    "px-3 py-1.5 text-xs rounded-md font-medium transition-colors duration-200 {}",
                                                                    if is_current {
                                                                        "bg-seafoam-500 text-white shadow-md"
                                                                    } else {
                                                                        "bg-white dark:bg-teal-600 text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-teal-500 border border-gray-300 dark:border-teal-500"
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
            </div>

            <div class="flex-1 overflow-y-auto overflow-x-hidden pr-2 min-w-0 w-full">
                <Suspense fallback=move || {
                    view! {
                        <div class="space-y-4 w-full overflow-hidden">
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-lg"></div>
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-lg"></div>
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-lg"></div>
                        </div>
                    }
                }>
                    {move || {
                        let messages = combined_messages();
                        if messages.is_empty() {
                            view! {
                                <div class="flex items-center justify-center h-32">
                                    <div class="text-center text-gray-500 dark:text-gray-400">
                                        <div class="text-lg mb-2">"💬"</div>
                                        <div class="text-sm">
                                            "No messages yet. Start a conversation!"
                                        </div>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="space-y-4 w-full overflow-hidden">
                                    <For
                                        each=move || combined_messages()
                                        key=|message| message.id()
                                        children=move |message| {
                                            let is_user = message.is_user();
                                            let is_streaming = message.is_streaming();
                                            view! {
                                                <div class=format!(
                                                    "flex w-full min-w-0 {}",
                                                    if is_user { "justify-end" } else { "justify-start" },
                                                )>
                                                    <div class=format!(
                                                        "max-w-[80%] min-w-0 rounded-lg p-4 shadow-sm overflow-hidden {} {}",
                                                        if is_user {
                                                            "bg-seafoam-500 text-white"
                                                        } else {
                                                            "bg-white dark:bg-teal-700 text-gray-800 dark:text-gray-200 border border-gray-200 dark:border-teal-600"
                                                        },
                                                        if is_streaming { "animate-pulse" } else { "" },
                                                    )>
                                                        <div class="prose prose-sm w-full max-w-full overflow-hidden">
                                                            {if is_user {
                                                                view! {
                                                                    <div class="whitespace-pre-wrap text-sm leading-relaxed text-white break-words w-full">
                                                                        <p class="whitespace-pre-wrap text-sm leading-relaxed text-white break-words w-full">
                                                                            {message.content().to_string()}
                                                                        </p>
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    <div class="text-sm leading-relaxed text-left w-full max-w-full overflow-hidden">
                                                                        <MarkdownRenderer
                                                                            content=message.content().to_string()
                                                                            class="text-left w-full max-w-full"
                                                                        />
                                                                    </div>
                                                                }.into_any()
                                                            }}

                                                        </div>

                                                        {move || {
                                                            if is_user && !is_streaming {
                                                                if let Some(db_id) = message.db_id() {
                                                                    view! {
                                                                        <div class="mt-3 pt-2 border-t border-white/20">
                                                                            <button
                                                                                class="px-3 py-1 text-xs bg-white/20 hover:bg-white/30 text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                                                                disabled=move || create_branch_action.pending().get()
                                                                                on:click=move |_| {
                                                                                    create_branch_action.dispatch((db_id,));
                                                                                }
                                                                            >

                                                                                {move || {
                                                                                    if create_branch_action.pending().get() {
                                                                                        "⏳ creating branch..."
                                                                                    } else {
                                                                                        "🌿 branch from here"
                                                                                    }
                                                                                }}

                                                                            </button>
                                                                        </div>
                                                                    }.into_any()
                                                                } else {
                                                                    view! {
                                                                        <div class="mt-3">
                                                                            <span></span>
                                                                        </div>
                                                                    }.into_any()
                                                                }
                                                            } else {
                                                                view! {
                                                                    <div class="mt-3">
                                                                        <span></span>
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                        }}

                                                    </div>
                                                </div>
                                            }
                                        }
                                    />

                                </div>
                            }.into_any()
                        }
                    }}

                </Suspense>
            </div>
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
