use leptos::prelude::*;
use leptos_icons::Icon;
use leptos_fetch::QueryClient;
use log::error;
use web_sys::Event;
use cfg_if::cfg_if;
use uuid::Uuid;

use crate::auth::{context::AuthContext, get_current_user};
use crate::models::conversations::ThreadView;
use crate::components::ui::{Button, IconButton, ButtonVariant, ButtonSize};

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
    // New props for project synchronization
    selected_project: ReadSignal<Option<Uuid>>,
    set_selected_project: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    let client: QueryClient = expect_context();
    let (search_query, set_search_query) = signal(String::new());
    let (is_search_focused, set_is_search_focused) = signal(false);
    let (_title_updates, _set_title_updates) = signal(std::collections::HashMap::<String, String>::new());
    let (_sse_connected, _set_sse_connected) = signal(false);
    let (hotkey_text, set_hotkey_text) = signal("Ctrl+K");

    // Node ref for the search input
    let search_input_ref = NodeRef::<leptos::html::Input>::new();

    // Set up SSE connection for title updates
    cfg_if! {
        if #[cfg(feature = "hydrate")] {
            Effect::new(move |_| {
                use wasm_bindgen_futures::spawn_local;
                use web_sys::{EventSource, MessageEvent, ErrorEvent};
                use wasm_bindgen::{prelude::*, JsCast};
                use crate::types::TitleUpdate;

                spawn_local(async move {
                    log::debug!("Setting up SSE connection for title updates");
                    
                    match EventSource::new("/api/title-updates") {
                        Ok(event_source) => {
                            _set_sse_connected.set(true);
                            
                            let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
                                if let Some(data) = e.data().as_string() {
                                    log::debug!("Received SSE message: {}", data);
                                    
                                    if let Ok(title_update) = serde_json::from_str::<TitleUpdate>(&data) {
                                        _set_title_updates.update(|updates| {
                                            updates.insert(title_update.thread_id.clone(), title_update.title.clone());
                                        });
                                    }
                                }
                            }) as Box<dyn FnMut(_)>);
                            
                            event_source.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
                            onmessage_callback.forget();
                            
                            let onerror_callback = Closure::wrap(Box::new(move |_: ErrorEvent| {
                                log::error!("SSE connection error for title updates");
                                _set_sse_connected.set(false);
                            }) as Box<dyn FnMut(_)>);
                            
                            event_source.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
                            onerror_callback.forget();
                        }
                        Err(e) => {
                            log::error!("Failed to create EventSource: {:?}", e);
                        }
                    }
                });
            });
        }
    }

    let threads_resource = client.resource(get_threads_query, || ());
    let search_resource = client.resource(search_threads_query, move || search_query.get());

    let current_threads = move || {
        if search_query.get().is_empty() {
            threads_resource.get()
        } else {
            search_resource.get()
        }
    };

    // Bidirectional sync: when user clicks on a project thread, auto-select the project
    let handle_thread_click = move |thread_id: String, thread: ThreadView| {
        set_current_thread_id.run(thread_id);
        
        // If this thread belongs to a project, auto-select that project
        if let Some(project_id) = thread.project_id {
            set_selected_project.set(Some(project_id));
        } else {
            // If it's not a project thread, clear project selection
            set_selected_project.set(None);
        }
    };

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
                        handle_thread_click(first_thread.id.clone(), first_thread.clone());
                        
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

    // find user agent on mount
    Effect::new(move |_| {
        if let Some(window) = web_sys::window() {
            if let Ok(ua) = window.navigator().user_agent() {
                let text = if ua.to_lowercase().contains("mac") { 
                    "⌘K" 
                } else { 
                    "Ctrl+K" 
                };
                set_hotkey_text.set(text);
            }
        }
    });

    // Global keyboard listener for Cmd+K / Ctrl+K
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

                    if current_id == thread_id {
                        match get_threads().await {
                            Ok(updated_threads) => {
                                if let Some(next_thread) = updated_threads.first() {
                                    handle_thread_click(next_thread.id.clone(), next_thread.clone());
                                } else {
                                    log::debug!("no threads left");
                                    set_current_thread_id.run(String::new());
                                    set_selected_project.set(None);
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

    view! {
        <div class="h-full flex flex-col surface-primary border-themed">
            <div class="flex-shrink-0 p-4 border-b border-themed">
                <div class="relative">
                    <input
                        node_ref=search_input_ref
                        type="text"
                        placeholder="grep threads..."
                        class="input-themed w-full pr-16"
                        prop:value=move || search_query.get()
                        on:input=handle_search
                        on:keydown=handle_search_keydown
                        on:focus=handle_focus
                        on:blur=handle_blur
                    />
                    <div class="absolute inset-y-0 right-0 flex items-center pr-3 pointer-events-none">
                        <span class=format!(
                            "text-xs font-mono px-1.5 py-0.5 rounded border transition-colors duration-150 {}",
                            if is_search_focused.get() {
                                "text-primary-600 bg-primary-50 border-primary-300"
                            } else {
                                "text-themed-secondary bg-surface-secondary border-themed"
                            },
                        )>{hotkey_text}</span>
                    </div>
                </div>
            </div>

            <div class="flex-1 overflow-y-auto scrollbar-themed">
                <Transition fallback=move || {
                    view! {
                        <div class="p-4">
                            <p class="text-themed-secondary text-sm">"Loading threads..."</p>
                        </div>
                    }
                        .into_any()
                }>
                    {move || {
                        match current_threads() {
                            Some(Ok(thread_list)) => {
                                if thread_list.is_empty() {
                                    view! {
                                        <div class="p-4">
                                            <p class="text-themed-secondary text-sm">
                                                "No threads found"
                                            </p>
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    let is_thread_highlighted = move |thread: &ThreadView| {
                                        let project_id = selected_project.get();
                                        match project_id {
                                            Some(pid) => thread.project_id == Some(pid),
                                            None => false,
                                        }
                                    };
                                    let tree_nodes = build_thread_tree(thread_list.clone());
                                    view! {
                                        <div class="p-2">
                                            <For
                                                each=move || tree_nodes.clone()
                                                key=|root_node| root_node.thread.id.clone()
                                                children=move |root_node| {
                                                    let value = thread_list.clone();
                                                    view! {
                                                        <ThreadTreeNode
                                                            node=root_node
                                                            current_thread_id=current_thread_id
                                                            set_current_thread_id=Callback::new(move |
                                                                thread_id: String|
                                                            {
                                                                if let Some(thread) = value
                                                                    .iter()
                                                                    .find(|t| t.id == thread_id)
                                                                {
                                                                    handle_thread_click(thread_id, thread.clone());
                                                                }
                                                            })

                                                            delete_action=delete_thread_action
                                                            depth=0
                                                            title_updates=_title_updates
                                                            is_highlighted=is_thread_highlighted
                                                        />
                                                    }
                                                        .into_any()
                                                }
                                            />

                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Some(Err(e)) => {
                                view! {
                                    <div class="p-4">
                                        <div class="error-themed text-sm">
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
    is_highlighted: impl Fn(&ThreadView) -> bool + Copy + 'static + Send + Sync,
) -> impl IntoView {
    let thread = node.thread.clone();
    let thread_id = thread.id.clone();
    let thread_id_for_memo = thread_id.clone();
    let thread_id_for_set = thread_id.clone();
    let thread_id_for_delete = thread_id.clone();
    let thread_for_display = thread.clone();
    let thread_for_generation = thread.clone();
    let thread_for_styles = thread.clone();
    let thread_for_title_display = thread.clone();
    let thread_for_highlight = thread.clone();
    
    // Calculate indentation based on depth
    let margin_left = format!("{}rem", depth as f32 * 1.5);
    
    // Use a memo for the active state to make it reactive properly
    let is_active = Memo::new(move |_| current_thread_id.get() == thread_id_for_memo);
    
    // Check if this thread should be highlighted (belongs to selected project)
    let should_highlight = Memo::new(move |_| is_highlighted(&thread_for_highlight));
    
    // Check if title is being generated
    let is_generating_title = Memo::new(move |_| {
        let updates = title_updates.get();
        let thread_id = thread_for_generation.id.as_str();
        
        log::debug!("Checking if thread {} is generating title", thread_id);
        
        if let Some(title) = updates.get(thread_id) {
            let is_generating = title.contains("Generating") || title.contains("...");
            log::debug!("Current title: '{}', is_generating: {}", title, is_generating);
            is_generating
        } else {
            log::debug!("No title update found for thread");
            false
        }
    });

    // Get display title with SSE updates
    let display_title = Memo::new(move |_| {
        let updates = title_updates.get();
        let thread_id = thread_for_title_display.id.as_str();
        
        // Check if we have a live title update
        if let Some(updated_title) = updates.get(thread_id) {
            // Don't show "Generating..." messages as the actual title
            if updated_title.contains("Generating") {
                thread_for_title_display.title.clone().unwrap_or_else(|| "New Thread".to_string())
            } else {
                updated_title.clone()
            }
        } else {
            thread_for_title_display.title.clone().unwrap_or_else(|| "New Thread".to_string())
        }
    });

    // Determine thread type and styling
    let thread_type = Memo::new(move |_| {
        let is_branch = thread_for_styles.parent_thread_id.is_some();
        let is_project = thread_for_styles.project_id.is_some();
        
        if is_branch {
            ThreadType::Branch
        } else if is_project {
            ThreadType::Project
        } else {
            ThreadType::Main
        }
    });

    let has_children = !node.children.is_empty();
    let children_for_each = node.children.clone();
    let children_for_last_check = node.children.clone();

    view! {
        <div class="thread-group">
            <div class="thread-item-container flex flex-col relative">
                {move || {
                    if depth > 0 {
                        view! {
                            <div class="absolute left-0 top-0 w-full h-full pointer-events-none">
                                <div
                                    class="absolute border-l-2 border-themed"
                                    style:left=format!("{}rem", (depth as f32 - 1.0) * 1.5 + 0.75)
                                    style:top="0"
                                    style:height=if is_last_child { "1.5rem" } else { "100%" }
                                ></div>

                                <div
                                    class="absolute border-t-2 border-themed"
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
                    class=format!(
                        "flex w-full justify-between items-center relative z-10 overflow-hidden rounded transition-all duration-200 {}",
                        if should_highlight.get() {
                            "bg-mint-100 dark:bg-mint-900 hover:bg-mint-200 dark:hover:bg-mint-800"
                        } else {
                            "hover:bg-surface-secondary"
                        },
                    )

                    style:margin-left=margin_left
                >
                    // Thread button container with ellipsis truncation
                    <div class="flex-1 min-w-0">
                        <ThreadItemButton
                            _thread=thread_for_display.clone()
                            is_active=is_active
                            is_generating=is_generating_title
                            display_title=display_title
                            thread_type=thread_type
                            is_highlighted=should_highlight
                            on_click=Callback::new(move |_| {
                                set_current_thread_id.run(thread_id_for_set.clone());
                            })
                        />

                    </div>

                    // Trash icon - appears on hover of this specific element
                    <div class="relative flex-shrink-0 group">
                        <IconButton
                            variant=ButtonVariant::Ghost
                            size=ButtonSize::Tiny
                            class="opacity-50 text-teal-700 dark:text-teal-100 group-hover:opacity-100 transition-opacity"
                            on_click=Callback::new(move |_| {
                                delete_action.dispatch(thread_id_for_delete.clone());
                            })
                        >

                            <Icon
                                icon=icondata_bs::BsTrash3
                                width="12"
                                height="12"
                                style="filter: brightness(0) saturate(100%) invert(36%) sepia(42%) saturate(1617%) hue-rotate(154deg) brightness(94%) contrast(89%);"
                            />
                        </IconButton>
                    </div>
                </div>
                {move || {
                    if has_children {
                        view! {
                            <div
                                class="absolute border-l-2 border-themed pointer-events-none"
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
                                    is_highlighted=is_highlighted
                                />
                            }
                                .into_any()
                        }
                    />

                </div>
            </div>
        </div>
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ThreadType {
    Main,
    Branch,
    Project,
}

#[component]
fn ThreadItemButton(
    _thread: ThreadView,
    is_active: Memo<bool>,
    is_generating: Memo<bool>,
    display_title: Memo<String>,
    thread_type: Memo<ThreadType>,
    is_highlighted: Memo<bool>,
    #[prop(into)] on_click: Callback<web_sys::MouseEvent>,
) -> impl IntoView {
    let get_icon_and_style = move || {
        let active = is_active.get();
        let highlighted = is_highlighted.get();
        let thread_type = thread_type.get();
        
        match (thread_type, active, highlighted) {
            // Active project thread (primary + highlighted)
            (ThreadType::Project, true, true) => (
                view! {
                    <div class="w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_bs::BsFolder2Open width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Success,
                "text-gray-200 border-0 border-mint-400 dark:border-mint-600"
            ),
            // Active project thread (not highlighted)
            (ThreadType::Project, true, false) => (
                view! {
                    <div class="w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_bs::BsFolder2Open width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Success,
                "text-white"
            ),
            // Highlighted project thread (not active)
            (ThreadType::Project, false, true) => (
                view! {
                    <div class="w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_bs::BsFolder2 width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Primary,
                "text-white bg-mint-600 dark:bg-mint-700"
            ),
            // Normal project thread
            (ThreadType::Project, false, false) => (
                view! {
                    <div class="w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_bs::BsFolder2 width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Outline,
                "text-themed-primary"
            ),
            // Branch threads
            (ThreadType::Branch, true, _) => (
                view! {
                    <div class="rotate-180-mirror w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_mdi::MdiSourceBranch width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Success,
                "text-white"
            ),
            (ThreadType::Branch, false, _) => (
                view! {
                    <div class="rotate-180-mirror w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_mdi::MdiSourceBranch width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Outline,
                "text-themed-primary"
            ),
            // Main threads
            (ThreadType::Main, true, _) => (
                view! {
                    <div class="w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_bs::BsChatRightDots width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Primary,
                "text-white"
            ),
            (ThreadType::Main, false, _) => (
                view! {
                    <div class="w-4 h-4 flex items-center justify-center">
                        <Icon icon=icondata_bs::BsChatLeft width="16" height="16"/>
                    </div>
                }.into_any(),
                ButtonVariant::Outline,
                "text-teal-700"
            ),
        }
    };

    view! {
        {move || {
            let (icon, variant, extra_class) = get_icon_and_style();
            let title = display_title.get();
            view! {
                <Button
                    variant=variant
                    size=ButtonSize::Small
                    full_width=true
                    class=format!(
                        "text-left text-sm justify-start gap-2 {} {}",
                        extra_class,
                        if is_generating.get() { "animate-pulse" } else { "" },
                    )

                    on_click=on_click
                >
                    {icon}
                    <span class="truncate ir">{title}</span>
                </Button>
            }
        }}
    }
}

#[component]
fn UserInfo() -> impl IntoView {
    let auth = use_context::<AuthContext>().expect("AuthContext not found");
    let current_user = Resource::new(|| (), |_| get_current_user());

    view! {
        <div class="border-t border-themed pt-3 mt-3 p-4">
            <Transition fallback=|| {
                view! {
                    <div class="flex items-center space-x-3 p-3 card-themed animate-pulse">
                        <div class="w-10 h-10 bg-surface-secondary rounded-full"></div>
                        <div class="flex-1 space-y-1">
                            <div class="h-4 bg-surface-secondary rounded"></div>
                            <div class="h-3 bg-surface-secondary rounded w-3/4"></div>
                        </div>
                    </div>
                }
            }>
                {move || {
                    if auth.is_loading.get() {
                        view! {
                            <div class="flex items-center space-x-3 p-3 card-themed animate-pulse">
                                <div class="w-10 h-10 bg-surface-secondary rounded-full"></div>
                                <div class="flex-1">
                                    <div class="text-sm text-themed-secondary">"Loading..."</div>
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
                                                class="flex items-center space-x-3 p-3 card-themed card-hover cursor-pointer group"
                                            >
                                                {user
                                                    .avatar_url
                                                    .as_ref()
                                                    .map(|avatar| {
                                                        view! {
                                                            <img
                                                                src=avatar.clone()
                                                                alt="User avatar"
                                                                class="w-10 h-10 rounded-full border-2 border-themed"
                                                            />
                                                        }
                                                            .into_any()
                                                    })
                                                    .unwrap_or_else(|| {
                                                        view! {
                                                            <div class="w-10 h-10 bg-primary-600 rounded-full flex items-center justify-center text-white text-lg">
                                                                {user
                                                                    .display_name
                                                                    .clone()
                                                                    .or(user.username.clone())
                                                                    .unwrap_or_else(|| "U".to_string())
                                                                    .chars()
                                                                    .next()
                                                                    .unwrap_or('U')
                                                                    .to_uppercase()
                                                                    .to_string()}
                                                            </div>
                                                        }
                                                            .into_any()
                                                    })}

                                                <div class="flex-1 min-w-0">
                                                    <p class="text-sm font-medium text-themed-primary truncate group-hover:opacity-80">
                                                        {user
                                                            .display_name
                                                            .clone()
                                                            .or(user.username.clone())
                                                            .unwrap_or_else(|| "Anonymous".to_string())}
                                                    </p>
                                                    <p class="text-xs text-themed-secondary group-hover:opacity-80">
                                                        "free"
                                                    </p>
                                                </div>
                                                <div class="text-themed-secondary group-hover:text-themed-primary">
                                                    "›"
                                                </div>
                                            </a>
                                        }
                                            .into_any()
                                    }
                                    Ok(None) => {
                                        view! {
                                            <Button
                                                variant=ButtonVariant::Success
                                                full_width=true
                                                class="justify-center"
                                                on_click=Callback::new(|_| {
                                                    if let Some(window) = web_sys::window() {
                                                        let _ = window.location().set_href("/admin");
                                                    }
                                                })
                                            >

                                                "Sign In"
                                            </Button>
                                        }
                                            .into_any()
                                    }
                                    Err(_) => {
                                        view! {
                                            <div class="flex items-center justify-center p-3 bg-danger-500 text-white rounded-lg text-sm">
                                                "Error loading user"
                                            </div>
                                        }
                                            .into_any()
                                    }
                                }
                            })
                            .unwrap_or_else(|| {
                                view! {
                                    <div class="flex items-center justify-center p-3 card-themed text-sm text-themed-secondary">
                                        "Loading user..."
                                    </div>
                                }
                                    .into_any()
                            })
                    } else {
                        view! {
                            <Button
                                variant=ButtonVariant::Success
                                full_width=true
                                class="justify-center"
                                on_click=Callback::new(|_| {
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().set_href("/admin");
                                    }
                                })
                            >

                                "Sign In"
                            </Button>
                        }
                            .into_any()
                    }
                }}

            </Transition>
        </div>
    }
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

// All the server functions remain the same
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

