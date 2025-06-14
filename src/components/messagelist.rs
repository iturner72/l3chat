use leptos::prelude::*;
use leptos_fetch::QueryClient;
use leptos_icons::Icon;
use chrono::Utc;
use std::borrow::Cow;
use wasm_bindgen::JsCast;

use crate::models::conversations::{MessageView, DisplayMessage, PendingMessage, BranchInfo};
use crate::components::markdown::MarkdownRenderer;

async fn get_messages_query(thread_id: String) -> Result<Vec<MessageView>, String> {
    if thread_id.is_empty() {
        Ok(Vec::new())
    } else {
        get_messages_for_thread(thread_id).await.map_err(|e| e.to_string())
    }
}

async fn get_branches_query(thread_id: String) -> Result<Vec<BranchInfo>, String> {
    if thread_id.is_empty() {
        Ok(Vec::new())
    } else {
        get_thread_branches(thread_id).await.map_err(|e| e.to_string())
    }
}

fn get_highlighted_segments(text: &str, search_term: &str) -> Vec<(String, bool)> {
    if search_term.is_empty() {
        return vec![(text.to_string(), false)];
    }

    let search_term = search_term.to_lowercase();
    let mut result = Vec::new();
    let mut last_index = 0;
    let text_lower = text.to_lowercase();

    while let Some(start_idx) = text_lower[last_index..].find(&search_term) {
        let absolute_start = last_index + start_idx;
        let absolute_end = absolute_start + search_term.len();

        // Add non-matching segment if there is one
        if absolute_start > last_index {
            result.push((text[last_index..absolute_start].to_string(), false));
        }

        // Add matching segment (using original case from text)
        result.push((text[absolute_start..absolute_end].to_string(), true));

        last_index = absolute_end;
    }

    // Add remaining text if any
    if last_index < text.len() {
        result.push((text[last_index..].to_string(), false));
    }

    result
}

fn message_contains_search_term(message: &DisplayMessage, search_term: &str) -> bool {
    if search_term.is_empty() {
        return false;
    }
    message.content().to_lowercase().contains(&search_term.to_lowercase())
}

#[component]
fn HighlightedText<'a>(
    #[prop(into)] text: Cow<'a, str>,
    #[prop(into)] search_term: String,
    #[prop(optional)] class: &'static str,
    #[prop(optional)] is_current_match: bool,
) -> impl IntoView {
    let segments = get_highlighted_segments(&text, &search_term);

    view! {
        <span class=class>
            {segments
                .into_iter()
                .map(|(text, is_highlight)| {
                    if is_highlight {
                        view! {
                            <mark class=format!(
                                "rounded px-0.5 {}",
                                if is_current_match {
                                    "bg-mint-300 dark:bg-mint-800 text-seafoam-900 dark:text-seafoam-100 ring-2 ring-mint-500 dark:ring-mint-400"
                                } else {
                                    "bg-mint-400 dark:bg-mint-900 text-seafoam-900 dark:text-seafoam-200"
                                },
                            )>{text}</mark>
                        }
                            .into_any()
                    } else {
                        view! { <span>{text}</span> }.into_any()
                    }
                })
                .collect_view()}
        </span>
    }
}

#[component]
pub fn MessageList(
    current_thread_id: ReadSignal<String>,
    set_current_thread_id: WriteSignal<String>,
    #[prop(optional)] refetch_trigger: Option<ReadSignal<i32>>,
    #[prop(optional)] pending_messages: Option<ReadSignal<Vec<PendingMessage>>>,
    #[prop(optional)] search_term: Option<ReadSignal<String>>,
    #[prop(optional)] search_action: Option<ReadSignal<bool>>,
) -> impl IntoView {
    let client: QueryClient = expect_context();
    
    // Search navigation state
    let (current_match_index, set_current_match_index) = signal(0);
    let (total_matches, set_total_matches) = signal(0);
    
    let _query_key = move || {
        let thread = current_thread_id.get();
        let trigger = refetch_trigger.map(|t| t.get()).unwrap_or(0);
        (thread, trigger)
    };

    let messages_resource = client.resource(
        get_messages_query, 
        move || current_thread_id.get()
    );

    let branches_resource = client.resource(
        get_branches_query,
        move || current_thread_id.get()
    );

    // Manually invalidate when refetch trigger changes
    Effect::new(move |_| {
        if let Some(trigger) = refetch_trigger {
            trigger.get();
            let thread_id = current_thread_id.get();
            client.invalidate_query(get_messages_query, &thread_id);
            client.invalidate_query(get_branches_query, &thread_id);
        }
    });

    let combined_messages = move || -> Vec<DisplayMessage> {
        let db_messages = messages_resource.get()
            .and_then(|result| result.ok())
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

    // Get messages with matches and update total count
    let messages_with_matches = move || -> Vec<(DisplayMessage, bool, usize, bool)> {
        let messages = combined_messages();
        let term = search_term.map(|s| s.get()).unwrap_or_default();
        let current_idx = current_match_index.get();
        
        let mut match_index = 0;
        let result: Vec<(DisplayMessage, bool, usize, bool)> = messages.into_iter()
            .map(|message| {
                let has_match = message_contains_search_term(&message, &term);
                let (this_match_index, is_current_match) = if has_match {
                    let idx = match_index;
                    let is_current = idx == current_idx;
                    match_index += 1;
                    (idx, is_current)
                } else {
                    (0, false)
                };
                (message, has_match, this_match_index, is_current_match)
            })
            .collect();
            
        set_total_matches.set(match_index);
        result
    };

    let scroll_to_message = move |message_id: String| {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(element) = document.get_element_by_id(&format!("message-{}", message_id)) {
                    element.scroll_into_view();
                }
            }
        }
    };

    let navigate_to_match = move |index: usize| {
        let messages_with_search = messages_with_matches();
        let matching_messages: Vec<_> = messages_with_search.into_iter()
            .filter(|(_, has_match, _, _)| *has_match)
            .collect();
            
        if let Some((message, _, _, _)) = matching_messages.get(index) {
            scroll_to_message(message.id());
        }
    };

    // Handle search action trigger (when Enter is pressed in ThreadList OR when thread changes with active search)
    Effect::new(move |_| {
        if let Some(action_signal) = search_action {
            action_signal.get(); // Subscribe to changes
            if action_signal.get() {
                // Navigate to first match
                set_current_match_index.set(0);
                navigate_to_match(0);
            }
        }
    });

    // Debug effect to track search term changes
    Effect::new(move |_| {
        let term = search_term.map(|s| s.get()).unwrap_or_default();
        let thread = current_thread_id.get();
        log::info!("MessageList: search_term='{}', thread_id='{}'", term, thread);
    });

    // Auto-apply search when thread changes and there's an active search term
    Effect::new(move |_| {
        let thread = current_thread_id.get();
        let term = search_term.map(|s| s.get()).unwrap_or_default();
        
        log::info!("Thread change effect: thread='{}', term='{}'", thread, term);
        
        if !thread.is_empty() && !term.is_empty() {
            // Reset match index when switching threads
            set_current_match_index.set(0);
            
            // Small delay to let messages load, then navigate to first match
            set_timeout(
                move || {
                    log::info!("Navigating to first match after thread switch");
                    navigate_to_match(0);
                },
                std::time::Duration::from_millis(500) // Increased delay to ensure messages are loaded
            );
        }
    });

    // Also trigger search navigation when messages finish loading
    Effect::new(move |_| {
        if let Some(Ok(_)) = messages_resource.get() {
            let term = search_term.map(|s| s.get()).unwrap_or_default();
            let matches = total_matches.get();
            
            if !term.is_empty() && matches > 0 && current_match_index.get() == 0 {
                // Small delay to ensure DOM is updated
                set_timeout(
                    move || {
                        navigate_to_match(0);
                    },
                    std::time::Duration::from_millis(100)
                );
            }
        }
    });

    // Keyboard navigation
    Effect::new(move |_| {
        use wasm_bindgen::closure::Closure;
        use web_sys::KeyboardEvent;
        
        let handle_keydown = {
            let navigate_to_match = navigate_to_match;
            Closure::wrap(Box::new(move |event: KeyboardEvent| {
                let term = search_term.map(|s| s.get()).unwrap_or_default();
                if term.is_empty() || total_matches.get() == 0 {
                    return;
                }

                // Cmd+N (or Win+J) - Next match
                if event.key() == "j" && (event.meta_key() || event.ctrl_key()) {
                    event.prevent_default();
                    let new_index = (current_match_index.get() + 1) % total_matches.get();
                    set_current_match_index.set(new_index);
                    navigate_to_match(new_index);
                }
                // Cmd+P (or Win+I) - Previous match
                else if event.key() == "i" && (event.meta_key() || event.ctrl_key()) {
                    event.prevent_default();
                    let new_index = if current_match_index.get() == 0 {
                        total_matches.get().saturating_sub(1)
                    } else {
                        current_match_index.get() - 1
                    };
                    set_current_match_index.set(new_index);
                    navigate_to_match(new_index);
                }
                // F3 - Next match (fallback)
                else if event.key() == "F3" && !event.shift_key() {
                    event.prevent_default();
                    let new_index = (current_match_index.get() + 1) % total_matches.get();
                    set_current_match_index.set(new_index);
                    navigate_to_match(new_index);
                }
                // Shift+F3 - Previous match (fallback)
                else if event.key() == "F3" && event.shift_key() {
                    event.prevent_default();
                    let new_index = if current_match_index.get() == 0 {
                        total_matches.get().saturating_sub(1)
                    } else {
                        current_match_index.get() - 1
                    };
                    set_current_match_index.set(new_index);
                    navigate_to_match(new_index);
                }
            }) as Box<dyn FnMut(KeyboardEvent)>)
        };

        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                let _ = document.add_event_listener_with_callback(
                    "keydown",
                    handle_keydown.as_ref().unchecked_ref()
                );
            }
        }

        handle_keydown.forget();
    });

    let create_branch_action = Action::new(move |(message_id,): &(i32,)| {
        let message_id = *message_id;
        let thread_id = current_thread_id.get();
        
        async move {
            match create_branch(thread_id.clone(), message_id, None).await {
                Ok(new_thread_id) => {
                    log::info!("Created branch: {}", new_thread_id);
                    set_current_thread_id.set(new_thread_id);
                    
                    // Invalidate queries to refresh data
                    let client: QueryClient = expect_context();
                    client.invalidate_query(get_messages_query, current_thread_id.get());
                    client.invalidate_query(get_branches_query, &thread_id);
                    client.invalidate_query(crate::components::threadlist::get_threads_query, ());
                    
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
                {move || {
                    let term = search_term.map(|s| s.get()).unwrap_or_default();
                    let matches = total_matches.get();
                    if !term.is_empty() && matches > 0 {
                        view! {
                            <div class="mb-3 p-3 bg-mint-100 dark:bg-mint-900 rounded-sm border border-mint-300 dark:border-mint-700">
                                <div class="flex items-center justify-between">
                                    <div class="flex items-center space-x-2">
                                        <span class="text-sm font-medium text-mint-800 dark:text-mint-200">
                                            {format!("\"{}\" - {} matches", term, matches)}
                                        </span>
                                        <span class="text-xs text-mint-600 dark:text-mint-400">
                                            {format!("({}/{})", current_match_index.get() + 1, matches)}
                                        </span>
                                    </div>
                                    <div class="flex items-center space-x-1">
                                        <button
                                            class="px-2 py-1 text-xs bg-mint-200 dark:bg-mint-800 hover:bg-mint-300 dark:hover:bg-mint-700 
                                            text-mint-800 dark:text-mint-200 rounded transition-colors"
                                            on:click=move |_| {
                                                let new_index = if current_match_index.get() == 0 {
                                                    total_matches.get().saturating_sub(1)
                                                } else {
                                                    current_match_index.get() - 1
                                                };
                                                set_current_match_index.set(new_index);
                                                navigate_to_match(new_index);
                                            }

                                            disabled=move || total_matches.get() <= 1
                                        >
                                            "↓"
                                        </button>
                                        <button
                                            class="px-2 py-1 text-xs bg-mint-200 dark:bg-mint-800 hover:bg-mint-300 dark:hover:bg-mint-700 
                                            text-mint-800 dark:text-mint-200 rounded transition-colors"
                                            on:click=move |_| {
                                                let new_index = (current_match_index.get() + 1)
                                                    % total_matches.get();
                                                set_current_match_index.set(new_index);
                                                navigate_to_match(new_index);
                                            }

                                            disabled=move || total_matches.get() <= 1
                                        >
                                            "↑"
                                        </button>
                                        <span class="text-xs text-mint-600 dark:text-mint-400 ml-2">
                                            "⌘J/⌘I • F3/⇧F3"
                                        </span>
                                    </div>
                                </div>
                            </div>
                        }
                            .into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}
                <Transition fallback=move || {
                    view! {
                        <div class="animate-pulse bg-gray-300 dark:bg-teal-600 h-8 rounded-md"></div>
                    }
                }>
                    {move || {
                        branches_resource
                            .get()
                            .map(|branches_result| {
                                match branches_result {
                                    Ok(branches) => {
                                        if !branches.is_empty() {
                                            view! {
                                                <div class="p-3 bg-gray-200 dark:bg-teal-700 rounded-sm border border-gray-300 dark:border-teal-600">
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

                                                                        <div class="inline-flex items-center gap-1">
                                                                            <div class="rotate-180-mirror">
                                                                                <Icon
                                                                                    icon=icondata::MdiSourceBranch
                                                                                    width="16"
                                                                                    height="16"
                                                                                />
                                                                            </div>
                                                                            {branch.branch_name.unwrap_or_else(|| "branch".to_string())}
                                                                        </div>
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
                                    }
                                    Err(_) => view! { <div></div> }.into_any(),
                                }
                            })
                            .unwrap_or_else(|| view! { <div></div> }.into_any())
                    }}

                </Transition>
            </div>

            <div class="flex-1 overflow-y-auto overflow-x-hidden pr-2 min-w-0 w-full">
                <Transition fallback=move || {
                    view! {
                        <div class="space-y-4 w-full overflow-hidden">
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-lg"></div>
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-lg"></div>
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-800 h-20 rounded-lg"></div>
                        </div>
                    }
                }>
                    {move || {
                        let messages_data = messages_with_matches();
                        if messages_data.is_empty() {
                            view! {
                                <div class="flex items-center justify-center h-32">
                                    <div class="text-center text-gray-500 dark:text-gray-400">
                                        <div class="text-lg mb-2">"💬"</div>
                                        <div class="text-sm">
                                            "No messages yet. Start a conversation!"
                                        </div>
                                    </div>
                                </div>
                            }
                                .into_any()
                        } else {
                            let highlight_term = search_term.map(|s| s.get()).unwrap_or_default();
                            view! {
                                <div class="space-y-4 w-full overflow-hidden">
                                    <For
                                        each=move || messages_with_matches()
                                        key=|(message, _, _, _)| message.id()
                                        children=move |
                                            (message, has_match, _match_index, is_current_match)|
                                        {
                                            let is_user = message.is_user();
                                            let is_streaming = message.is_streaming();
                                            let search_highlight_term = highlight_term.clone();
                                            let message_id = message.id();
                                            view! {
                                                <div
                                                    id=format!("message-{}", message_id)
                                                    class=format!(
                                                        "flex w-full min-w-0 {} {}",
                                                        if is_user { "justify-end" } else { "justify-start" },
                                                        if is_current_match {
                                                            "ring-0 ring-mint-500 dark:ring-aqua-700 ring-opacity-100 rounded-sm shadow-lg"
                                                        } else if has_match {
                                                            "ring-0 ring-mint-600 dark:ring-aqua-800 ring-opacity-50 rounded-sm"
                                                        } else {
                                                            ""
                                                        },
                                                    )
                                                >

                                                    <div class=format!(
                                                        "max-w-[80%] min-w-0 rounded-lg p-4 shadow-sm overflow-hidden {} {} {}",
                                                        if is_user {
                                                            "bg-seafoam-500 text-white"
                                                        } else {
                                                            "bg-white dark:bg-teal-700 text-gray-800 dark:text-gray-200 border border-gray-200 dark:border-teal-600"
                                                        },
                                                        if is_streaming { "animate-pulse" } else { "" },
                                                        if is_current_match {
                                                            "ring-0 ring-mint-500 dark:ring-aqua-700 ring-inset"
                                                        } else {
                                                            ""
                                                        },
                                                    )>

                                                        {}
                                                        <div class="prose prose-sm w-full max-w-full overflow-hidden">
                                                            {if is_user {
                                                                view! {
                                                                    <div class="whitespace-pre-wrap text-sm leading-relaxed text-white break-words w-full">
                                                                        <HighlightedText
                                                                            text=Cow::from(message.content().to_string())
                                                                            search_term=search_highlight_term.clone()
                                                                            class=if is_current_match {
                                                                                "whitespace-pre-wrap text-sm leading-relaxed text-white break-words w-full bg-white/10 rounded px-1"
                                                                            } else {
                                                                                "whitespace-pre-wrap text-sm leading-relaxed text-white break-words w-full"
                                                                            }
                                                                        />

                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                view! {
                                                                    <div class=format!(
                                                                        "text-sm leading-relaxed text-left w-full max-w-full overflow-hidden {}",
                                                                        if is_current_match {
                                                                            "bg-mint-50 dark:bg-mint-900/20 rounded p-2"
                                                                        } else {
                                                                            ""
                                                                        },
                                                                    )>
                                                                        {if !search_highlight_term.is_empty() {
                                                                            view! {
                                                                                <HighlightedText
                                                                                    text=Cow::from(message.content().to_string())
                                                                                    search_term=search_highlight_term.clone()
                                                                                    class="text-left w-full max-w-full whitespace-pre-wrap text-sm leading-relaxed break-words"
                                                                                />
                                                                            }
                                                                                .into_any()
                                                                        } else {
                                                                            view! {
                                                                                <MarkdownRenderer
                                                                                    content=message.content().to_string()
                                                                                    class="text-left w-full max-w-full"
                                                                                />
                                                                            }
                                                                                .into_any()
                                                                        }}

                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            }}

                                                        </div>
                                                        {}
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

                                                                                <div class="inline-flex items-center gap-1">
                                                                                    <div class="rotate-180-mirror">
                                                                                        <Icon
                                                                                            icon=icondata::MdiSourceBranchPlus
                                                                                            width="16"
                                                                                            height="16"
                                                                                        />
                                                                                    </div>
                                                                                    {move || {
                                                                                        if create_branch_action.pending().get() {
                                                                                            "creating branch..."
                                                                                        } else {
                                                                                            ""
                                                                                        }
                                                                                    }}

                                                                                </div>
                                                                            </button>
                                                                        </div>
                                                                    }
                                                                        .into_any()
                                                                } else {
                                                                    view! {
                                                                        <div class="mt-3">
                                                                            <span></span>
                                                                        </div>
                                                                    }
                                                                        .into_any()
                                                                }
                                                            } else {
                                                                view! {
                                                                    <div class="mt-3">
                                                                        <span></span>
                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            }
                                                        }}

                                                    </div>
                                                </div>
                                            }
                                        }
                                    />

                                </div>
                            }
                                .into_any()
                        }
                    }}

                </Transition>
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

#[server(CreateBranch, "/api")]
pub async fn create_branch(
    source_thread_id: String,
    branch_point_message_id: i32,
    _branch_name: Option<String>,
) -> Result<String, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::{RunQueryDsl, AsyncConnection};
    use std::fmt;
    use std::error::Error;
    use crate::state::AppState;
    use crate::models::conversations::{Thread, Message, NewMessage};
    use crate::schema::{threads, messages};
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum BranchError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
        NotFound,
    }

    impl fmt::Display for BranchError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                BranchError::Pool(e) => write!(f, "pool error: {e}"),
                BranchError::Database(e) => write!(f, "database error: {e}"),
                BranchError::Unauthorized => write!(f, "unauthorized - user not logged in"),
                BranchError::NotFound => write!(f, "source thread or message not found"),
            }
        }
    }

    impl Error for BranchError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                BranchError::Database(e) => Some(e),
                _ => None,
            }
        }
    }

    impl From<diesel::result::Error> for BranchError {
        fn from(error: diesel::result::Error) -> Self {
            BranchError::Database(error)
        }
    }

    let current_user = get_current_user().await.map_err(|_| BranchError::Unauthorized)?;
    let user_id = current_user.ok_or(BranchError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| BranchError::Pool(e.to_string()))?;

    let new_thread_id = uuid::Uuid::new_v4().to_string();

    let source_thread_id_clone = source_thread_id.clone();
    let new_thread_id_clone = new_thread_id.clone();

    let result = conn.transaction(|conn| {
        Box::pin(async move {
            // Verify source thread exists and user owns it
            let _source_thread = threads::table
                .find(&source_thread_id_clone)
                .filter(threads::user_id.eq(user_id))
                .first::<Thread>(conn)
                .await
                .optional()?
                .ok_or(BranchError::NotFound)?;
    
            // Get the first message ID of the source thread to check if we're branching from the start
            let first_message_id = messages::table
                .filter(messages::thread_id.eq(&source_thread_id_clone))
                .order(messages::id.asc())
                .select(messages::id)
                .first::<i32>(conn)
                .await
                .optional()?;
    
            let messages_to_copy = if let Some(first_id) = first_message_id {
                if branch_point_message_id == first_id {
                    // Branching from the very first message - create a fresh start with no messages
                    Vec::new()
                } else {
                    // Normal branching - get messages before the branch point
                    messages::table
                        .filter(messages::thread_id.eq(&source_thread_id_clone))
                        .filter(messages::id.lt(branch_point_message_id))
                        .order(messages::id.asc())
                        .load::<Message>(conn)
                        .await?
                }
            } else {
                // Source thread has no messages - nothing to copy
                Vec::new()
            };
    
            // Get ALL branch names for this user to find the highest number used
            let all_branch_names: Vec<Option<String>> = threads::table
                .filter(threads::user_id.eq(user_id))
                .filter(threads::parent_thread_id.is_not_null()) // Only branches
                .select(threads::branch_name)
                .load(conn)
                .await?;
    
            // Find the highest existing branch number across all user's branches
            let mut highest_branch_number = 0;
            for branch_name_opt in all_branch_names {
                if let Some(branch_name) = branch_name_opt {
                    if let Ok(num) = branch_name.parse::<i32>() {
                        if num > highest_branch_number {
                            highest_branch_number = num;
                        }
                    }
                }
            }
    
            // Generate next sequential branch name
            let branch_name = format!("{}", highest_branch_number + 1);
    
            // Create new thread
            let new_thread = Thread {
                id: new_thread_id_clone.clone(),
                created_at: Some(chrono::Utc::now().naive_utc()),
                updated_at: Some(chrono::Utc::now().naive_utc()),
                user_id: Some(user_id),
                parent_thread_id: Some(source_thread_id_clone.clone()),
                branch_point_message_id: Some(branch_point_message_id),
                branch_name: Some(branch_name),
                title: None,
            };
    
            diesel::insert_into(threads::table)
                .values(&new_thread)
                .execute(conn)
                .await?;
    
            // Copy messages to new thread with new IDs (if any)
            for message in messages_to_copy {
                let new_message = NewMessage {
                    thread_id: new_thread_id_clone.clone(),
                    content: message.content,
                    role: message.role,
                    active_model: message.active_model,
                    active_lab: message.active_lab,
                    user_id: Some(user_id),
                };
    
                diesel::insert_into(messages::table)
                    .values(&new_message)
                    .execute(conn)
                    .await?;
            }
    
            Ok::<String, BranchError>(new_thread_id_clone)
        })
    })
    .await?;

    log::info!("Created branch {} from thread {} at message {}", result, source_thread_id, branch_point_message_id);
    Ok(result)
}

#[server(GetThreadBranches, "/api")]
pub async fn get_thread_branches(thread_id: String) -> Result<Vec<BranchInfo>, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;
    use std::error::Error;
    use crate::state::AppState;
    use crate::models::conversations::Thread;
    use crate::schema::threads;
    use crate::auth::get_current_user;
    
    #[derive(Debug)]
    enum BranchError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
    }
    
    impl fmt::Display for BranchError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                BranchError::Pool(e) => write!(f, "pool error: {e}"),
                BranchError::Database(e) => write!(f, "database error: {e}"),
                BranchError::Unauthorized => write!(f, "unauthorized - user not logged in"),
            }
        }
    }
    
    impl Error for BranchError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                BranchError::Database(e) => Some(e),
                _ => None,
            }
        }
    }
    
    let current_user = get_current_user().await.map_err(|_| BranchError::Unauthorized)?;
    let user_id = current_user.ok_or(BranchError::Unauthorized)?.id;
    
    let app_state = use_context::<AppState>()
        .expect("failed to get AppState from context");
    
    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| BranchError::Pool(e.to_string()))?;
    
    // Find all branches of this thread, ordered by branch_name as integer
    let mut branches = threads::table
        .filter(threads::parent_thread_id.eq(&thread_id))
        .filter(threads::user_id.eq(user_id))
        .order(threads::created_at.desc())
        .load::<Thread>(&mut conn)
        .await
        .map_err(BranchError::Database)?;

    // Sort by branch_name as integers (1, 2, 3, etc.)
    branches.sort_by(|a, b| {
        let a_num: i32 = a.branch_name.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0);
        let b_num: i32 = b.branch_name.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0);
        a_num.cmp(&b_num)
    });
    
    // Convert to BranchInfo with simplified data
    let branch_infos: Vec<BranchInfo> = branches
        .into_iter()
        .map(|branch| BranchInfo {
            thread_id: branch.id,
            branch_name: branch.branch_name,
            model: "mixed".to_string(), // Since branches can have multiple models
            lab: "mixed".to_string(),   // Since branches can have multiple labs
            created_at: branch.created_at.map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)),
        })
        .collect();
    
    Ok(branch_infos)
}
