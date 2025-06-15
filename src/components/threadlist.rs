use leptos::prelude::*;
use leptos_icons::Icon;
use leptos_fetch::QueryClient;
use log::error;
use web_sys::Event;
use cfg_if::cfg_if;

use crate::auth::{context::AuthContext, get_current_user};
use crate::models::conversations::ThreadView;

pub async fn get_threads_query() -> Result<Vec<ThreadView>, String> {
    get_threads().await.map_err(|e| e.to_string())
}

async fn search_threads_query(query: String) -> Result<Vec<ThreadView>, String> {
    if query.is_empty() {
        get_threads().await.map_err(|e| e.to_string())
    } else {
        search_threads(query).await.map_err(|e| e.to_string())
    }
}

#[component]
pub fn ThreadList(
    current_thread_id: ReadSignal<String>,
    #[prop(into)] set_current_thread_id: Callback<String>,
    #[prop(optional)] set_search_term: Option<WriteSignal<String>>,
    #[prop(optional)] set_search_action: Option<WriteSignal<bool>>,
) -> impl IntoView {
    let client: QueryClient = expect_context();
    let (search_query, set_search_query) = signal(String::new());
    let (is_search_focused, set_is_search_focused) = signal(false);
    let (title_updates, _set_title_updates) = signal(std::collections::HashMap::<String, String>::new());
    let (_sse_connected, _set_sse_connected) = signal(false);

    // Node ref for the search input
    let search_input_ref = NodeRef::<leptos::html::Input>::new();

    let threads_resource = client.resource(get_threads_query, || ());
    let search_resource = client.resource(search_threads_query, move || search_query.get());

    let handle_search = move |ev: Event| {
        let query = event_target_value(&ev);
        set_search_query.set(query.clone());
        
        if let Some(set_term) = set_search_term {
            set_term.set(query);
        }
    };

    let handle_search_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" {
            ev.prevent_default();
            let query = search_query.get();
            
            if !query.is_empty() {
                if let Some(set_term) = set_search_term {
                    set_term.set(query.clone());
                }
                
                if let Some(Ok(threads)) = search_resource.get() {
                    if let Some(first_thread) = threads.first() {
                        set_current_thread_id.run(first_thread.id.clone());
                        
                        if let Some(set_action) = set_search_action {
                            set_timeout(
                                move || {
                                    set_action.set(true);
                                    set_timeout(
                                        move || set_action.set(false),
                                        std::time::Duration::from_millis(100)
                                    );
                                },
                                std::time::Duration::from_millis(50)
                            );
                        }
                    }
                }
            }
        } else if ev.key() == "Escape" {
            set_search_query.set(String::new());
            if let Some(set_term) = set_search_term {
                set_term.set(String::new());
            }
            if let Some(input) = search_input_ref.get() {
                let _ = input.blur();
            }
        }
    };

    let handle_focus = move |_| {
        set_is_search_focused.set(true);
    };

    let handle_blur = move |_| {
        set_is_search_focused.set(false);
    };

    // Global keyboard listener - only on client side
    cfg_if! {
        if #[cfg(feature = "hydrate")] {
            use wasm_bindgen::{closure::Closure, JsCast};
            use web_sys::HtmlInputElement;
            
            Effect::new(move |_| {
                let handle_global_keydown = {
                    let search_input_ref = search_input_ref.clone();
                    Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
                        // Check for Cmd+K (Mac) or Ctrl+K (Windows/Linux)
                        if event.key() == "k" && (event.meta_key() || event.ctrl_key()) {
                            event.prevent_default();
                            
                            if let Some(input) = search_input_ref.get() {
                                let input_element = input.unchecked_ref::<HtmlInputElement>();
                                
                                // Check if the input is currently focused
                                if let Some(active_element) = web_sys::window()
                                    .and_then(|w| w.document())
                                    .and_then(|d| d.active_element())
                                {
                                    if active_element == input_element.clone().into() {
                                        // Input is focused, blur it (toggle off)
                                        let _ = input_element.blur();
                                    } else {
                                        // Input is not focused, focus it (toggle on)
                                        let _ = input_element.focus();
                                        let _ = input_element.select(); // Select all text for easy replacement
                                    }
                                } else {
                                    // No active element, focus the input
                                    let _ = input_element.focus();
                                    let _ = input_element.select();
                                }
                            }
                        }
                    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>)
                };

                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        let _ = document.add_event_listener_with_callback(
                            "keydown",
                            handle_global_keydown.as_ref().unchecked_ref()
                        );
                    }
                }

                handle_global_keydown.forget();
            });
        }
    }

    let delete_thread_action = Action::new(move |thread_id: &String| {
        let thread_id = thread_id.clone();
        let current_id = current_thread_id.get_untracked(); 
        async move {
            match delete_thread(thread_id.clone()).await {
                Ok(_) => {
                    let client: QueryClient = expect_context();
                    client.invalidate_query(get_threads_query, ());
                    client.invalidate_query(search_threads_query, search_query.get_untracked());

                    if current_id.to_string() == thread_id {
                        match get_threads().await {
                            Ok(updated_threads) => {
                                if let Some(next_thread) = updated_threads.first() {
                                    set_current_thread_id.run(next_thread.id.clone());
                                } else {
                                    log::debug!("no threads left");
                                    set_current_thread_id.run(String::new());
                                }
                            }
                            Err(e) => {
                                error!("failed to fetch updated threads: {e:?}");
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("failed to delete thread: {e:?}");
                }
            }
        }
    });

    let current_threads = move || {
        if search_query.get().is_empty() {
            threads_resource.get()
        } else {
            search_resource.get()
        }
    };

    // Platform detection - only on client side
    let get_hotkey_text = move || {
        cfg_if! {
            if #[cfg(feature = "hydrate")] {
                web_sys::window()
                    .and_then(|w| w.navigator().user_agent().ok())
                    .map(|ua| if ua.to_lowercase().contains("mac") { "⌘K" } else { "Ctrl+K" })
                    .unwrap_or("Ctrl+K")
            } else {
                "Ctrl+K" // Default for SSR
            }
        }
    };

    cfg_if::cfg_if! {
        if #[cfg(feature = "hydrate")] {
            use leptos::task::spawn_local;
            spawn_local(async move {
                use std::rc::Rc;
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;
                use web_sys::{EventSource, MessageEvent, ErrorEvent};

                let event_source = Rc::new(
                    EventSource::new("/api/title-updates")
                        .expect("Failed to connect to title updates")
                );

                let on_open = {
                    Closure::wrap(Box::new(move |_: web_sys::Event| {
                        _set_sse_connected.set(true);
                    }) as Box<dyn FnMut(_)>)
                };

                let on_message = {
                    Closure::wrap(Box::new(move |event: MessageEvent| {
                        if let Some(data) = event.data().as_string() {
                            if let Ok(update) = serde_json::from_str::<crate::types::TitleUpdate>(&data) {
                                _set_title_updates.update(|updates| {
                                    updates.insert(update.thread_id.clone(), update.title.clone());
                                });
                                
                                if update.status == "completed" {
                                    set_timeout(
                                        move || {
                                            client.invalidate_query(get_threads_query, ());
                                        },
                                        std::time::Duration::from_millis(500)
                                    );
                                }
                            }
                        }
                    }) as Box<dyn FnMut(_)>)
                };

                let on_error = {
                    Closure::wrap(Box::new(move |_: ErrorEvent| {
                        _set_sse_connected.set(false);
                    }) as Box<dyn FnMut(_)>)
                };

                event_source.set_onopen(Some(on_open.as_ref().unchecked_ref()));
                event_source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
                event_source.set_onerror(Some(on_error.as_ref().unchecked_ref()));

                on_open.forget();
                on_message.forget();
                on_error.forget();
            });
        }
    }
    
    log::debug!("SSE RenderEffect created successfully");

    view! {
        <div class="thread-list-container flex flex-col h-full">
            <div class="flex-shrink-0">
                <div class="relative flex items-center w-full">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        class="h-5 w-5 absolute left-3 text-gray-400 dark:text-teal-500"
                        viewBox="0 1 20 20"
                        fill="currentColor"
                    >
                        <path
                            fill-rule="evenodd"
                            d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z"
                            clip-rule="evenodd"
                        ></path>
                    </svg>

                    <input
                        node_ref=search_input_ref
                        type="text"
                        placeholder="grep your threads"
                        on:input=handle_search
                        on:keydown=handle_search_keydown
                        on:focus=handle_focus
                        on:blur=handle_blur
                        prop:value=search_query
                        class=move || {
                            format!(
                                "grep-box w-full pl-10 pr-16 p-2 mb-2 bg-gray-100 dark:bg-teal-800 text-teal-600 dark:text-mint-400
                            border-0 transition duration-0 ease-in-out {}",
                                if is_search_focused.get() {
                                    "border-teal-500 dark:border-mint-300 ring-2 ring-teal-500/20 dark:ring-mint-300/20 shadow-md"
                                } else {
                                    "border-gray-300 dark:border-teal-600 focus:border-teal-500 dark:focus:border-mint-300 focus:outline-none focus:ring-2 focus:ring-teal-500/20 dark:focus:ring-mint-300/20"
                                },
                            )
                        }
                    />

                    <div class="absolute right-3 flex items-center">
                        <span class=move || {
                            format!(
                                "text-xs font-mono px-1.5 py-0.5 rounded border transition-colors duration-0 {}",
                                if is_search_focused.get() {
                                    "text-teal-600 dark:text-mint-300 bg-teal-100 dark:bg-teal-600 border-teal-300 dark:border-mint-400"
                                } else {
                                    "text-gray-400 dark:text-teal-500 bg-gray-200 dark:bg-teal-700 border-gray-300 dark:border-teal-600"
                                },
                            )
                        }>{get_hotkey_text}</span>
                    </div>
                </div>
            </div>

            <div class="flex-1 overflow-y-auto">
                <Transition fallback=move || {
                    view! {
                        <div class="w-full">
                            <p class="text-gray-500 dark:text-gray-400 text-sm">
                                "Loading threads..."
                            </p>
                        </div>
                    }
                }>
                    {move || {
                        match current_threads() {
                            Some(Ok(thread_list)) => {
                                if thread_list.is_empty() {
                                    view! {
                                        <div class="w-full">
                                            <p class="text-gray-500 dark:text-gray-400 text-sm">
                                                "No threads found"
                                            </p>
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    let tree_nodes = build_thread_tree(thread_list);
                                    view! {
                                        <For
                                            each=move || tree_nodes.clone()
                                            key=|root_node| root_node.thread.id.clone()
                                            children=move |root_node| {
                                                view! {
                                                    <ThreadTreeNode
                                                        node=root_node
                                                        current_thread_id=current_thread_id
                                                        set_current_thread_id=set_current_thread_id
                                                        delete_action=delete_thread_action
                                                        depth=0
                                                        title_updates=title_updates
                                                    />
                                                }
                                            }
                                        />
                                    }
                                        .into_any()
                                }
                            }
                            Some(Err(e)) => {
                                view! {
                                    <div class="w-full">
                                        <div class="text-salmon-500 text-sm">
                                            "Error loading threads: " {e}
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                            None => view! { <div></div> }.into_any(),
                        }
                    }}

                </Transition>
            </div>

            <div class="flex-shrink-0">
                <UserInfo/>
            </div>
        </div>
    }
}

#[component]
fn ThreadTreeNode(
    node: ThreadNode,
    current_thread_id: ReadSignal<String>,
    #[prop(into)] set_current_thread_id: Callback<String>,
    delete_action: Action<String, ()>,
    depth: usize,
    #[prop(optional)] is_last_child: bool,
    title_updates: ReadSignal<std::collections::HashMap<String, String>>,
) -> impl IntoView {
    let thread = node.thread.clone();
    let thread_id = thread.id.clone();
    let thread_id_for_memo = thread_id.clone();
    let thread_id_for_set = thread_id.clone();
    let thread_id_for_delete = thread_id.clone();
    let thread_for_display = thread.clone();
    let thread_for_generation = thread.clone();
    let thread_for_styles = thread.clone();
    
    // Calculate indentation based on depth
    let margin_left = format!("{}rem", depth as f32 * 1.5);
    
    // Use a memo for the active state to make it reactive properly
    let is_active = Memo::new(move |_| current_thread_id.get().to_string() == thread_id_for_memo);
    
    // Determine styling based on whether it's a main thread or branch AND if it's active
    let get_styles = move || {
        let active = is_active.get();
        let is_branch = thread_for_styles.parent_thread_id.is_some();
        let is_project = thread_for_styles.project_id.is_some();
        
        if is_branch {
            // This is a branch - use branch icon
            if active {
                (
                    view! {
                        <div class="rotate-180-mirror">
                            <Icon icon=icondata::MdiSourceBranch width="16" height="16"/>
                        </div>
                    }.into_any(),
                    "border-seafoam-500 bg-seafoam-600 dark:bg-seafoam-700",
                    "ir text-sm text-white group-hover:text-white",
                )
            } else {
                (
                    view! {
                        <div class="rotate-180-mirror">
                            <Icon icon=icondata::MdiSourceBranch width="16" height="16"/>
                        </div>
                    }.into_any(),
                    "border-gray-600 bg-gray-200 dark:bg-teal-700 hover:border-seafoam-600 hover:bg-gray-300 dark:hover:bg-teal-600",
                    "ir text-sm text-gray-600 group-hover:text-gray-800 dark:text-gray-300 dark:group-hover:text-white",
                )
            }
        } else if is_project {
            if active {
                (
                    view! { <span class="text-blue-200">{"📁"}</span> }.into_any(),
                    "border-seafoam-500 bg-seafoam-600 dark:bg-seafoam-700",
                    "ir text-sm text-gray-500 group-hover:text-gray-700",
                )
            } else {
                (
                    view! { <span class="text-blue-600 dark:text-blue-400">{"📁"}</span> }.into_any(),
                    "border-seafoam-700 bg-gray-300 dark:bg-teal-800 hover:border-seafoam-800 hover:bg-gray-400 dark:hover:bg-gray-700",
                    "ir text-sm text-gray-700 group-hover:text-gray-500 dark:text-gray-100 dark:group-hover:text-gray-500",
                )
            }
        } else {
            // This is a main thread - no icon
            if active {
                (
                    view! { <span></span> }.into_any(),
                    "border-teal-500 bg-teal-600 dark:bg-teal-700",
                    "ir text-sm text-white group-hover:text-white",
                )
            } else {
                (
                    view! { <span></span> }.into_any(),
                    "border-teal-700 bg-gray-300 dark:bg-teal-800 hover:border-teal-800 hover:bg-gray-400 dark:hover:bg-gray-700",
                    "ir text-sm text-gray-700 group-hover:text-white dark:text-gray-100 dark:group-hover:text-white",
                )
            }
        }
    };

    let display_name = Memo::new(move |_| {
        let updates = title_updates.get();
        let thread = &thread_for_display;
        
        // Check for live updates FIRST
        if let Some(live_title) = updates.get(&thread.id) {
            return live_title.clone();
        }
        
        // Check for branch naming
        if let Some(branch_name) = &thread.branch_name {
            format!("branch {branch_name}")
        } else if thread.parent_thread_id.is_some() {
            "branch".to_string()
        } else {
            // Root thread logic
            let base_title = if let Some(title) = &thread.title {
                if !title.trim().is_empty() {
                    title.clone()
                } else {
                    if thread.id.len() > 24 {
                        format!("{}...", &thread.id[..24])
                    } else {
                        thread.id.clone()
                    }
                }
            } else {
                if thread.id.len() > 24 {
                    format!("{}...", &thread.id[..24])
                } else {
                    thread.id.clone()
                }
            };
    
            // For project threads, show project name in the title
            if let Some(project_name) = &thread.project_name {
                // Truncate project name if too long
                let truncated_project = if project_name.len() > 15 {
                    format!("{}...", &project_name[..12])
                } else {
                    project_name.clone()
                };
                
                // Show just the project name for cleaner look since we have the folder icon
                truncated_project
            } else {
                base_title
            }
        }
    });

    let is_generating_title = Memo::new(move |_| {
        let updates = title_updates.get();
        let thread_id = &thread_for_generation.id;
        
        log::debug!("Checking if thread {thread_id} is generating title");
        
        let x = if let Some(title) = updates.get(thread_id) {
            let is_generating = title.contains("Generating") || title.contains("...");
            log::debug!("Current title: '{title:?}', is_generating: {is_generating:?}");
            is_generating
        } else {
            log::debug!("No title update found for thread");
            false
        };

        x 
    });

    let has_children = !node.children.is_empty();
    let children_for_each = node.children.clone();
    let children_for_last_check = node.children.clone();

    view! {
        <div class="thread-group mb-1">
            <div class="thread-item-container flex flex-col relative">
                {move || {
                    if depth > 0 {
                        view! {
                            <div class="absolute left-0 top-0 w-full h-full pointer-events-none">
                                <div
                                    class="absolute border-l-2 border-gray-400 dark:border-gray-600"
                                    style:left=format!("{}rem", (depth as f32 - 1.0) * 1.5 + 0.75)
                                    style:top="0"
                                    style:height=if is_last_child { "1.5rem" } else { "100%" }
                                ></div>

                                <div
                                    class="absolute border-t-2 border-gray-400 dark:border-gray-600"
                                    style:left=format!("{}rem", (depth as f32 - 1.0) * 1.5 + 0.75)
                                    style:top="1.5rem"
                                    style:width="0.75rem"
                                ></div>
                            </div>
                        }
                            .into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}
                <div
                    class="flex w-full justify-between items-center relative z-10"
                    style:margin-left=margin_left
                >
                    {move || {
                        let (icon, button_class, text_class) = get_styles();
                        let thread_id_for_click = thread_id_for_set.clone();
                        let is_generating = is_generating_title.get();
                        view! {
                            <button
                                class=format!(
                                    "thread-item w-full p-2 border-0 {} rounded-md transition duration-0 ease-in-out group text-sm relative {}",
                                    button_class,
                                    if is_generating { "animate-pulse" } else { "" },
                                )

                                on:click=move |_| {
                                    log::debug!("Clicked thread: {thread_id_for_click}");
                                    set_current_thread_id.run(thread_id_for_click.clone());
                                }
                            >

                                <div class="flex items-center">
                                    <span class="mr-2">{icon}</span>
                                    <div class="flex items-center space-x-2 flex-1 min-w-0">
                                        <p class=format!(
                                            "thread-name {} transition duration-0 ease-in-out truncate flex-1 text-left",
                                            text_class,
                                        )>{move || display_name.get()}</p>
                                        {move || {
                                            if is_generating {
                                                view! {
                                                    <div class="flex-shrink-0">
                                                        <svg
                                                            class="animate-spin h-3 w-3 text-current"
                                                            xmlns="http://www.w3.org/2000/svg"
                                                            fill="none"
                                                            viewBox="0 0 24 24"
                                                        >
                                                            <circle
                                                                class="opacity-25"
                                                                cx="12"
                                                                cy="12"
                                                                r="10"
                                                                stroke="currentColor"
                                                                stroke-width="4"
                                                            ></circle>
                                                            <path
                                                                class="opacity-75"
                                                                fill="currentColor"
                                                                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                                                            ></path>
                                                        </svg>
                                                    </div>
                                                }
                                                    .into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }
                                        }}

                                    </div>
                                </div>
                            </button>
                        }
                    }}

                    <button
                        class="delete-button ib text-teal-600 dark:text-mint-400 hover:text-teal-400 dark:hover:text-mint-300 text-sm ml-2 p-2 
                        bg-gray-400 dark:bg-teal-900 hover:bg-gray-500 dark:hover:bg-teal-800 rounded transition duration-0 ease-in-out relative z-10"
                        on:click=move |_| {
                            delete_action.dispatch(thread_id_for_delete.clone());
                        }
                    >

                        "x"
                    </button>
                </div>
                {move || {
                    if has_children {
                        view! {
                            <div
                                class="absolute border-l-2 border-gray-400 dark:border-gray-600 pointer-events-none"
                                style:left=format!("{}rem", depth as f32 * 1.5 + 0.75)
                                style:top="3rem"
                                style:bottom="0"
                            ></div>
                        }
                            .into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}
                <div class="children-container">
                    <For
                        each=move || children_for_each.clone()
                        key=|child| child.thread.id.clone()
                        children=move |child_node| {
                            let is_last = {
                                let children = children_for_last_check.clone();
                                let child_id = child_node.thread.id.clone();
                                children
                                    .last()
                                    .map(|last| last.thread.id == child_id)
                                    .unwrap_or(false)
                            };
                            view! {
                                <ThreadTreeNode
                                    node=child_node
                                    current_thread_id=current_thread_id
                                    set_current_thread_id=set_current_thread_id
                                    delete_action=delete_action
                                    depth=depth + 1
                                    is_last_child=is_last
                                    title_updates=title_updates
                                />
                            }
                        }
                    />

                </div>
            </div>
        </div>
    }.into_any()
}

// Helper function to build thread tree structure - make it deterministic
fn build_thread_tree(threads: Vec<ThreadView>) -> Vec<ThreadNode> {
    let mut all_nodes: std::collections::BTreeMap<String, ThreadNode> = threads
        .into_iter()
        .map(|thread| {
            let id = thread.id.clone();
            (id, ThreadNode {
                thread,
                children: Vec::new(),
            })
        })
        .collect();

    // Collect parent-child relationships and sort them for deterministic order
    let mut relationships: Vec<(String, String)> = Vec::new(); // (child_id, parent_id)
    for node in all_nodes.values() {
        if let Some(parent_id) = &node.thread.parent_thread_id {
            relationships.push((node.thread.id.clone(), parent_id.clone()));
        }
    }
    
    // Sort relationships for deterministic processing
    relationships.sort();

    // Move children to their parents
    for (child_id, parent_id) in relationships {
        if all_nodes.contains_key(&parent_id) {
            if let Some(child_node) = all_nodes.remove(&child_id) {
                if let Some(parent_node) = all_nodes.get_mut(&parent_id) {
                    parent_node.children.push(child_node);
                }
            }
        }
    }

    // Sort children within each parent by creation time for consistency
    for node in all_nodes.values_mut() {
        node.children.sort_by(|a, b| {
            a.thread.created_at.cmp(&b.thread.created_at)
        });
    }

    // Collect remaining nodes (these are roots) and sort by creation time
    let mut roots: Vec<ThreadNode> = all_nodes.into_values().collect();
    roots.sort_by(|a, b| {
        b.thread.created_at.cmp(&a.thread.created_at)
    });

    roots
}

#[derive(Debug, Clone)]
struct ThreadNode {
    thread: ThreadView,
    children: Vec<ThreadNode>,
}

#[component]
fn UserInfo() -> impl IntoView {
    let auth = use_context::<AuthContext>().expect("AuthContext not found");
    let current_user = Resource::new(|| (), |_| get_current_user());

    view! {
        <div class="border-t border-gray-400 dark:border-teal-600 pt-3 mt-3">
            <Transition fallback=|| {
                view! {
                    <div class="flex items-center space-x-3 p-3 bg-gray-100 dark:bg-teal-700 rounded-lg">
                        <div class="w-10 h-10 bg-gray-300 dark:bg-teal-600 rounded-full animate-pulse"></div>
                        <div class="flex-1 space-y-1">
                            <div class="h-4 bg-gray-300 dark:bg-teal-600 rounded animate-pulse"></div>
                            <div class="h-3 bg-gray-300 dark:bg-teal-600 rounded w-3/4 animate-pulse"></div>
                        </div>
                    </div>
                }
            }>
                {move || {
                    if auth.is_loading.get() {
                        view! {
                            <div class="flex items-center space-x-3 p-3 bg-gray-100 dark:bg-teal-700 rounded-lg">
                                <div class="w-10 h-10 bg-gray-300 dark:bg-teal-600 rounded-full animate-pulse"></div>
                                <div class="flex-1">
                                    <div class="text-sm text-gray-500 dark:text-gray-400">
                                        "Loading..."
                                    </div>
                                </div>
                            </div>
                        }
                            .into_any()
                    } else if auth.is_authenticated.get() {
                        current_user
                            .get()
                            .map(|user_result| {
                                match user_result {
                                    Ok(Some(user)) => {
                                        view! {
                                            <a
                                                href="/admin-panel"
                                                class="flex items-center space-x-3 p-3 bg-gray-100 dark:bg-teal-700 rounded-lg hover:bg-gray-200 dark:hover:bg-teal-600 transition-colors cursor-pointer group"
                                            >
                                                {user
                                                    .avatar_url
                                                    .as_ref()
                                                    .map(|avatar| {
                                                        view! {
                                                            <img
                                                                src=avatar.clone()
                                                                alt="User avatar"
                                                                class="w-10 h-10 rounded-full border-2 border-gray-300 dark:border-teal-500"
                                                            />
                                                        }
                                                            .into_any()
                                                    })
                                                    .unwrap_or_else(|| {
                                                        view! {
                                                            <div class="w-10 h-10 bg-gray-300 dark:bg-teal-500 rounded-full flex items-center justify-center text-gray-600 dark:text-gray-300">
                                                                "👤"
                                                            </div>
                                                        }
                                                            .into_any()
                                                    })}

                                                <div class="flex-1 min-w-0">
                                                    <p class="text-sm font-medium text-gray-800 dark:text-gray-200 truncate group-hover:text-gray-900 dark:group-hover:text-white">
                                                        {user
                                                            .display_name
                                                            .clone()
                                                            .or(user.username.clone())
                                                            .unwrap_or_else(|| "Anonymous".to_string())}
                                                    </p>
                                                    <p class="text-xs text-gray-500 dark:text-gray-400 group-hover:text-gray-600 dark:group-hover:text-gray-300">
                                                        "free"
                                                    </p>
                                                </div>
                                                <div class="text-gray-400 dark:text-gray-500 group-hover:text-gray-600 dark:group-hover:text-gray-300">
                                                    "›"
                                                </div>
                                            </a>
                                        }
                                            .into_any()
                                    }
                                    Ok(None) => {
                                        view! {
                                            <a
                                                href="/admin"
                                                class="flex items-center justify-center p-3 bg-seafoam-500 dark:bg-seafoam-600 text-white rounded-lg hover:bg-seafoam-600 dark:hover:bg-seafoam-700 transition-colors"
                                            >
                                                "Sign In"
                                            </a>
                                        }
                                            .into_any()
                                    }
                                    Err(_) => {
                                        view! {
                                            <div class="flex items-center justify-center p-3 bg-salmon-100 dark:bg-salmon-900 text-salmon-600 dark:text-salmon-400 rounded-lg text-sm">
                                                "Error loading user"
                                            </div>
                                        }
                                            .into_any()
                                    }
                                }
                            })
                            .unwrap_or_else(|| {
                                view! {
                                    <div class="flex items-center justify-center p-3 bg-gray-200 dark:bg-teal-700 rounded-lg text-sm text-gray-500 dark:text-gray-400">
                                        "Loading user..."
                                    </div>
                                }
                                    .into_any()
                            })
                    } else {
                        view! {
                            <a
                                href="/admin"
                                class="flex items-center justify-center p-3 bg-seafoam-500 dark:bg-seafoam-600 text-white rounded-lg hover:bg-seafoam-600 dark:hover:bg-seafoam-700 transition-colors"
                            >
                                "Sign In"
                            </a>
                        }
                            .into_any()
                    }
                }}

            </Transition>
        </div>
    }
}

#[server(SearchThreads, "/api")]
pub async fn search_threads(query: String) -> Result<Vec<ThreadView>, ServerFnError> {
    use chrono::DateTime;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::schema::{threads, messages, projects};
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum SearchError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
    }

    impl fmt::Display for SearchError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                SearchError::Pool(e) => write!(f, "pool error: {e}"),
                SearchError::Database(e) => write!(f, "database error: {e}"),
                SearchError::Unauthorized => write!(f, "unauthorized - user not logged in"),
            }
        }
    }

    impl From<SearchError> for ServerFnError {
        fn from(error: SearchError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    fn to_server_error(e: SearchError) -> ServerFnError {
        ServerFnError::ServerError(e.to_string())
    }

    let current_user = get_current_user().await.map_err(|_| SearchError::Unauthorized)?;
    let other_user_id = current_user.ok_or(SearchError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| SearchError::Pool(e.to_string()))
        .map_err(to_server_error)?;

    let result = threads::table
        .left_join(messages::table.on(messages::thread_id.eq(threads::id)))
        .left_join(projects::table.on(threads::project_id.eq(projects::id.nullable())))
        .filter(threads::user_id.eq(other_user_id))
        .filter(
            threads::id.ilike(format!("%{query}%"))
                .or(messages::content.ilike(format!("%{query}%")))
                .or(projects::name.ilike(format!("%{query}%")))
        )
        .select((
            threads::id,
            threads::created_at,
            threads::updated_at,
            threads::user_id,
            threads::parent_thread_id,
            threads::branch_point_message_id,
            threads::branch_name,
            threads::title,
            threads::project_id,
            projects::name.nullable(),
        ))
        .distinct()
        .order(threads::created_at.desc())
        .load::<(
            String, 
            Option<chrono::NaiveDateTime>, 
            Option<chrono::NaiveDateTime>, 
            Option<i32>, 
            Option<String>, 
            Option<i32>, 
            Option<String>, 
            Option<String>, 
            Option<uuid::Uuid>, 
            Option<String>
        )>(&mut conn)
        .await
        .map_err(SearchError::Database)
        .map_err(to_server_error)?;

    let threads: Vec<ThreadView> = result
        .into_iter()
        .map(|(id, created_at, updated_at, user_id, parent_thread_id, branch_point_message_id, branch_name, title, project_id, project_name)| {
            ThreadView {
                id,
                created_at: created_at.map(|dt| DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)),
                updated_at: updated_at.map(|dt| DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)),
                user_id,
                parent_thread_id,
                branch_point_message_id,
                branch_name,
                title,
                project_id,
                project_name,
            }
        })
        .collect();

    Ok(threads)
}

#[server(DeleteThread, "/api")]
pub async fn delete_thread(thread_id: String) -> Result<(), ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::{RunQueryDsl, AsyncConnection};
    use crate::schema::{threads, messages};
    use std::fmt;
    use crate::state::AppState;
    
    #[derive(Debug)]
    enum ThreadError {
        Pool(String),
        Database(diesel::result::Error),
    }
    
    impl fmt::Display for ThreadError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ThreadError::Pool(e) => write!(f, "pool error: {e}"),
                ThreadError::Database(e)=> write!(f, "database error: {e}"),
            }
        }
    }
    
    fn to_server_error(e: ThreadError) -> ServerFnError {
        ServerFnError::ServerError(e.to_string())
    }
    
    fn delete_thread_recursive<'a>(
        conn: &'a mut diesel_async::AsyncPgConnection, 
        thread_id: &'a str
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), diesel::result::Error>> + Send + 'a>> {
        Box::pin(async move {

        let child_threads: Vec<String> = threads::table
            .filter(threads::parent_thread_id.eq(thread_id))
            .select(threads::id)
            .load(conn)
            .await?;
        
        // Recursively delete child threads
        for child_thread_id in child_threads {
            delete_thread_recursive(conn, &child_thread_id).await?;
        }
        
        // Get all message IDs that belong to this thread
        let message_ids: Vec<i32> = messages::table
            .filter(messages::thread_id.eq(thread_id))
            .select(messages::id)
            .load(conn)
            .await?;
        
        // Update any threads that reference these messages
        if !message_ids.is_empty() {
            diesel::update(
                threads::table.filter(
                    threads::branch_point_message_id.eq_any(&message_ids)
                )
            )
            .set(threads::branch_point_message_id.eq(None::<i32>))
            .execute(conn)
            .await?;
        }
        
        // Delete all messages associated with this thread
        diesel::delete(messages::table.filter(messages::thread_id.eq(thread_id)))
            .execute(conn)
            .await?;
        
        // Finally, delete the thread itself
        diesel::delete(threads::table.find(thread_id))
            .execute(conn)
            .await?;
        
        Ok(())
        })
    }
    
    let app_state = use_context::<AppState>()
        .expect("failed to get AppState from context");
    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| ThreadError::Pool(e.to_string()))
        .map_err(to_server_error)?;
    
    conn.transaction(|conn| {
        Box::pin(async move {
            delete_thread_recursive(conn, &thread_id).await
        })
    })
    .await
    .map_err(ThreadError::Database)
    .map_err(to_server_error)?;
    
    Ok(())
}

#[server(GetThreads, "/api")]
pub async fn get_threads() -> Result<Vec<ThreadView>, ServerFnError> {
    use chrono::DateTime;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;
    use uuid::Uuid;

    use crate::state::AppState;
    use crate::schema::{threads, projects};
    use crate::auth::get_current_user;

    #[derive(Debug)]
    enum ThreadError {
        Pool(String),
        Database(diesel::result::Error),
        Unauthorized,
    }

    impl fmt::Display for ThreadError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ThreadError::Pool(e) => write!(f, "Pool error: {e}"),
                ThreadError::Database(e) => write!(f, "Database error: {e}"),
                ThreadError::Unauthorized => write!(f, "Unauthorized"),
            }
        }
    }

    impl From<ThreadError> for ServerFnError {
        fn from(error: ThreadError) -> Self {
            ServerFnError::ServerError(error.to_string())
        }
    }

    fn to_server_error(e: ThreadError) -> ServerFnError {
        ServerFnError::ServerError(e.to_string())
    }

    let current_user = get_current_user().await.map_err(|_| ThreadError::Unauthorized)?;
    let user_id = current_user.ok_or(ThreadError::Unauthorized)?.id;

    let app_state = use_context::<AppState>()
        .expect("Failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| ThreadError::Pool(e.to_string()))
        .map_err(to_server_error)?;

    let result = threads::table
        .left_join(projects::table.on(threads::project_id.eq(projects::id.nullable())))
        .filter(threads::user_id.eq(user_id))
        .select((
            threads::id,
            threads::created_at,
            threads::updated_at,
            threads::user_id,
            threads::parent_thread_id,
            threads::branch_point_message_id,
            threads::branch_name,
            threads::title,
            threads::project_id,
            projects::name.nullable(), // Project name
        ))
        .order(threads::created_at.desc())
        .load::<(String, Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>, Option<i32>, Option<String>, Option<i32>, Option<String>, Option<String>, Option<Uuid>, Option<String>)>(&mut conn)
        .await
        .map_err(ThreadError::Database)
        .map_err(to_server_error)?;

    let threads: Vec<ThreadView> = result
        .into_iter()
        .map(|(id, created_at, updated_at, user_id, parent_thread_id, branch_point_message_id, branch_name, title, project_id, project_name)| {
            ThreadView {
                id,
                created_at: created_at.map(|dt| DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)),
                updated_at: updated_at.map(|dt| DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)),
                user_id,
                parent_thread_id,
                branch_point_message_id,
                branch_name,
                title,
                project_id,
                project_name,
            }
        })
        .collect();

    Ok(threads)
}

#[server(GetThreadBranches, "/api")]
pub async fn get_thread_branches(thread_id: String) -> Result<Vec<crate::models::conversations::BranchInfo>, ServerFnError> {
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
    let branch_infos: Vec<crate::models::conversations::BranchInfo> = branches
        .into_iter()
        .map(|branch| crate::models::conversations::BranchInfo {
            thread_id: branch.id,
            branch_name: branch.branch_name,
            model: "mixed".to_string(), // Since branches can have multiple models
            lab: "mixed".to_string(),   // Since branches can have multiple labs
            created_at: branch.created_at.map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)),
        })
        .collect();
    
    Ok(branch_infos)
}

