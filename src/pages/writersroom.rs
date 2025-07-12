use leptos::{prelude::*, task::spawn_local};
use leptos_icons::Icon;
use leptos_fetch::QueryClient;
use uuid::Uuid;

use crate::components::auth_nav::AuthNav;
use crate::components::chat::Chat;
use crate::components::projects::ProjectsPage;
use crate::components::threadlist::ThreadList;
use crate::components::messagelist::MessageList;
use crate::components::toast::Toast;
use crate::models::conversations::PendingMessage;
use crate::models::projects::ProjectView;
use crate::server_fn::projects::{get_user_projects, create_project_thread};

async fn create_project_thread_query(project_id: Uuid) -> Result<String, String> {
    create_project_thread(project_id).await.map_err(|e| e.to_string())
}

async fn create_thread_query() -> Result<String, String> {
    create_thread().await.map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct ThreadContext {
    pub set_thread_id: WriteSignal<String>,
    pub set_message_refetch_trigger: WriteSignal<i32>,
    pub set_pending_messages: WriteSignal<Vec<PendingMessage>>,
    pub set_search_term: WriteSignal<String>,
    pub title_updates: ReadSignal<std::collections::HashMap<String, String>>,
}

#[component]
pub fn WritersRoom() -> impl IntoView {
    let client: QueryClient = expect_context();
    
    let (show_threads, set_show_threads) = signal(false);
    let (show_projects, set_show_projects) = signal(false);
    let (thread_id, set_thread_id) = signal(Uuid::new_v4().to_string());
    let (toast_visible, set_toast_visible) = signal(false);
    let (toast_message, set_toast_message) = signal(String::new());
    let (message_refetch_trigger, set_message_refetch_trigger) = signal(0);
    let (search_term, set_search_term) = signal(String::new());
    let (search_action, set_search_action) = signal(false);
    let (pending_messages, set_pending_messages) = signal(Vec::<PendingMessage>::new());

    // title updates - the core signal which powers everything
    let (title_updates, set_title_updates) = signal(std::collections::HashMap::<String, String>::new());
    let (sse_connected, set_sse_connected) = signal(false);

    // floating notifications for completed titles
    let (recent_title_updates, set_recent_title_updates) = signal(Vec::<TitleNotification>::new());
    
    // Project selection state - this is the key addition
    let (selected_project, set_selected_project) = signal(None::<Uuid>);
    let (_available_projects, set_available_projects) = signal(Vec::<ProjectView>::new());

    cfg_if::cfg_if! {
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
                            set_sse_connected.set(true);
                            
                            let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
                                if let Some(data) = e.data().as_string() {
                                    log::debug!("Received SSE message: {}", data);
                                    
                                    if let Ok(title_update) = serde_json::from_str::<TitleUpdate>(&data) {
                                        // this single update will trigger ALL UI location
                                        // simultaneously
                                        set_title_updates.update(|updates| {
                                            updates.insert(title_update.thread_id.clone(), title_update.title.clone());
                                        });
                                    }
                                }
                            }) as Box<dyn FnMut(_)>);
                            
                            event_source.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
                            onmessage_callback.forget();
                            
                            let onerror_callback = Closure::wrap(Box::new(move |_: ErrorEvent| {
                                log::error!("SSE connection error for title updates");
                                set_sse_connected.set(false);
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

    // monitor title_updates for completed titles and create floating notifications
    Effect::new(move |_| {
        let updates = title_updates.get();

        for (thread_id, title) in updates.iter() {
            // only notify for completed titles (not generating ones)
            if !title.contains("Generating") && !title.contains("...") && !title.is_empty() && title != "New Thread" {
                // check if we already notified about this exact title
                let already_notified = recent_title_updates.get()
                    .iter()
                    .any(|notif| &notif.thread_id == thread_id && &notif.title == title);

                if !already_notified {
                    let notification = TitleNotification {
                        id: uuid::Uuid::new_v4().to_string(),
                        thread_id: thread_id.clone(),
                        title: title.clone(),
                        created_at: chrono::Utc::now(),
                    };

                    let notif_id = notification.id.clone();

                    set_recent_title_updates.update(|notifications| {
                        notifications.push(notification);
                        // keep only the last 3 notifications
                        if notifications.len() > 3 {
                            notifications.remove(0);
                        }
                    });

                    // auto-remove notification after 4 seconds
                    set_timeout(
                        move || {
                            set_recent_title_updates.update(|notifications| {
                                notifications.retain(|n| n.id != notif_id);
                            });
                        },
                        std::time::Duration::from_secs(4)
                    );
                }
            }
        }
    });

    let thread_context = ThreadContext {
        set_thread_id,
        set_message_refetch_trigger,
        set_pending_messages,
        set_search_term,
        title_updates,
    };
    provide_context(thread_context);

    Effect::new(move |_| {
        spawn_local(async move {
            match get_user_projects().await {
                Ok(projects) => set_available_projects.set(projects),
                Err(e) => log::error!("Failed to load projects: {e}"),
            }
        });
    });

    let create_thread_action = Action::new(move |project_id: &Option<Uuid>| {
        let project_id = *project_id;
        async move {
            let new_thread_id = if let Some(proj_id) = project_id {
                create_project_thread_query(proj_id).await?
            } else {
                create_thread_query().await?
            };
            set_thread_id(new_thread_id.clone());
            set_toast_message(format!("New thread created: {new_thread_id}"));
            set_toast_visible(true);

            client.invalidate_query(crate::components::threadlist::get_threads_query, ());
            
            set_message_refetch_trigger.update(|n| *n += 1);
            set_pending_messages.update(|msgs| msgs.clear());
            
            set_search_term.set(String::new());
            
            set_timeout(
                move || set_toast_visible(false),
                std::time::Duration::from_secs(3)
            );
            
            Ok(())
        }
    });

    let on_message_created = Callback::new(move |_: ()| {
        set_message_refetch_trigger.update(|n| *n += 1);
    });

    let on_thread_created = Callback::new(move |_new_thread_id: String| {
        client.invalidate_query(crate::components::threadlist::get_threads_query, ());

        set_message_refetch_trigger.update(|n| *n += 1);

        set_toast_message.set("Started new conversation".to_string());
        set_toast_visible.set(true);

        set_timeout(
            move || set_toast_visible(false),
            std::time::Duration::from_secs(5)
        );
    });

    let thread_switch_callback = Callback::new(move |new_id: String| {
        set_thread_id.set(new_id);
        set_message_refetch_trigger.update(|n| *n += 1);
        set_pending_messages.update(|msgs| msgs.clear());
        
        // Only close panels on mobile if we're not in search mode
        if search_term.get().is_empty() {
            if cfg!(feature = "hydrate") {
                let is_mobile = web_sys::window()
                    .and_then(|w| w.inner_width().ok())
                    .and_then(|w| w.as_f64())
                    .map(|w| w < 768.0)
                    .unwrap_or(false);
                
                if is_mobile {
                    set_show_threads.set(false);
                }
            }
        }
    });

    Effect::new(move |_| {
        if let Some(Err(error)) = create_thread_action.value().get() {
            set_toast_message(error);
            set_toast_visible(true);
            
            set_timeout(
                move || set_toast_visible(false),
                std::time::Duration::from_secs(5)
            );
        }
    });

    view! {
        <div class="w-full h-screen bg-gray-300 dark:bg-teal-900 flex flex-col">
            // Header - responsive navigation
            <div class="flex-shrink-0 p-2 border-b border-gray-400 dark:border-teal-700">
                <div class="flex flex-row items-center justify-between">
                    <div class="flex flex-row items-center justify-center space-x-2">
                        {move || {
                            let current_thread = thread_id.get();
                            let updates = title_updates.get();
                            if let Some(title_update) = updates.get(&current_thread) {
                                if title_update.contains("Generating")
                                    || title_update.contains("...")
                                {
                                    view! {
                                        <div class="hidden md:flex items-center px-3 py-1 bg-mint-200 darg:bg-mint-800 text-mint-800 dark:text-mint-200 rounded text-xs animate-pulse">
                                            <Icon icon=icondata_bs::BsStars width="12" height="12"/>
                                            <span class="ml-1">"AI thinking..."</span>
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }
                            } else {
                                view! { <div></div> }.into_any()
                            }
                        }}
                        // Mobile: Show hamburger menu for threads
                        <button
                            class="md:hidden text-xs text-teal-700 dark:text-teal-100 hover:text-gray-800 dark:hover:text-gray-200 
                            px-3 py-2 bg-gray-400 dark:bg-teal-700 hover:bg-gray-500 dark:hover:bg-teal-600 
                            border border-gray-600 dark:border-gray-500 hover:border-gray-800 dark:hover:border-gray-400 
                            rounded transition-colors duration-0 flex items-center justify-center"
                            on:click=move |_| {
                                set_show_threads.update(|v| *v = !*v);
                                set_show_projects.set(false);
                            }
                        >

                            <Icon
                                icon=icondata_bs::BsMenuDown
                                width="16"
                                height="16"
                                style="filter: brightness(0) saturate(100%) invert(36%) sepia(42%) saturate(1617%) hue-rotate(154deg) brightness(94%) contrast(89%);"
                            />
                        // Mobile: Projects toggle
                        </button>
                        <button
                            class="md:hidden text-xs text-teal-700 dark:text-teal-100 hover:text-gray-800 dark:hover:text-gray-200 
                            px-3 py-2 bg-gray-400 dark:bg-teal-700 hover:bg-gray-500 dark:hover:bg-teal-600 
                            border border-gray-600 dark:border-gray-500 hover:border-gray-800 dark:hover:border-gray-400 
                            rounded transition-colors duration-0 flex items-center justify-center"
                            on:click=move |_| {
                                set_show_projects.update(|v| *v = !*v);
                                set_show_threads.set(false);
                            }
                        >

                            <Icon
                                icon=icondata_bs::BsFolder
                                width="16"
                                height="16"
                                style="filter: brightness(0) saturate(100%) invert(36%) sepia(42%) saturate(1617%) hue-rotate(154deg) brightness(94%) contrast(89%);"
                            />
                        // Desktop: Original threads toggle
                        </button>
                        <button
                            class="hidden md:block text-xs md:text-sm text-teal-700 dark:text-teal-100 hover:text-gray-800 dark:hover:text-gray-200 
                            px-3 py-2 bg-gray-400 dark:bg-teal-700 hover:bg-gray-500 dark:hover:bg-teal-600 
                            border border-gray-600 dark:border-gray-500 hover:border-gray-800 dark:hover:border-gray-400 
                            rounded transition-colors duration-0 flex items-center justify-center"
                            on:click=move |_| set_show_threads.update(|v| *v = !*v)
                        >
                            {move || {
                                if show_threads.get() {
                                    view! {
                                        <Icon
                                            icon=icondata_tb::TbLayoutSidebarRightExpandFilled
                                            width="16"
                                            height="16"
                                            style="filter: brightness(0) saturate(100%) invert(36%) sepia(42%) saturate(1617%) hue-rotate(154deg) brightness(94%) contrast(89%);"
                                        />
                                    }
                                } else {
                                    view! {
                                        <Icon
                                            icon=icondata_tb::TbLayoutSidebarLeftExpandFilled
                                            width="16"
                                            height="16"
                                            style="filter: brightness(0) saturate(100%) invert(36%) sepia(42%) saturate(1617%) hue-rotate(154deg) brightness(94%) contrast(89%);"
                                        />
                                    }
                                }
                            }}

                        </button>
                        <button
                            class="text-xs md:text-sm text-teal-700 dark:text-teal-100 hover:text-teal-600 dark:hover:text-teal-200 
                            px-3 py-2 bg-gray-400 dark:bg-teal-700 hover:bg-gray-500 dark:hover:bg-teal-600 
                            border border-gray-600 dark:border-gray-500 hover:border-gray-800 dark:hover:border-gray-400 
                            rounded transition-colors duration-0 flex items-center justify-center"
                            disabled=move || create_thread_action.pending().get()
                            on:click=move |_| {
                                create_thread_action.dispatch(selected_project.get());
                            }
                        >

                            <Icon
                                icon=icondata_fi::FiPlus
                                width="16"
                                height="16"
                                style="filter: brightness(0) saturate(100%) invert(36%) sepia(42%) saturate(1617%) hue-rotate(154deg) brightness(94%) contrast(89%);"
                            />
                            <span class="hidden sm:inline">
                                {move || {
                                    if create_thread_action.pending().get() {
                                        if selected_project.get().is_some() {
                                            " Creating Project Chat..."
                                        } else {
                                            " Creating..."
                                        }
                                    } else {
                                        if selected_project.get().is_some() {
                                            " New Project Chat"
                                        } else {
                                            ""
                                        }
                                    }
                                }}

                            </span>
                        // Search indicator - hide on very small screens
                        </button>
                        <div class="hidden sm:block">
                            {move || {
                                let term = search_term.get();
                                if !term.is_empty() {
                                    view! {
                                        <div class="flex items-center px-3 py-2 bg-mint-200 dark:bg-mint-800 text-mint-800 dark:text-mint-200 rounded text-xs">
                                            <span class="mr-2">"🔍 \"" {term.clone()} "\""</span>
                                            <button
                                                class="text-mint-600 dark:text-mint-400 hover:text-mint-800 dark:hover:text-mint-200"
                                                on:click=move |_| set_search_term.set(String::new())
                                            >
                                                "×"
                                            </button>
                                            <span class="ml-2 text-xs text-mint-600 dark:text-mint-400 hidden md:inline">
                                                {move || {
                                                    cfg_if::cfg_if! {
                                                        if #[cfg(feature = "hydrate")] { web_sys::window()
                                                        .and_then(| w | w.navigator().user_agent().ok()).map(| ua |
                                                        if ua.to_lowercase().contains("mac") {
                                                        "⌘K to focus search" } else { "Ctrl+K to focus search" })
                                                        .unwrap_or("Ctrl+K to focus search") } else {
                                                        "Ctrl+K to focus search" }
                                                    }
                                                }}

                                            </span>
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="hidden md:flex items-center px-3 py-2 text-xs text-gray-500 dark:text-gray-400">
                                            {move || {
                                                cfg_if::cfg_if! {
                                                    if #[cfg(feature = "hydrate")] { web_sys::window()
                                                    .and_then(| w | w.navigator().user_agent().ok()).map(| ua |
                                                    if ua.to_lowercase().contains("mac") {
                                                    "⌘K to search threads" } else { "Ctrl+K to search threads"
                                                    }).unwrap_or("Ctrl+K to search threads") } else {
                                                    "Ctrl+K to search threads" }
                                                }
                                            }}

                                        </div>
                                    }
                                        .into_any()
                                }
                            }}

                        </div>
                    </div>

                    <AuthNav/>
                </div>
            </div>

            // Main content area
            <div class="flex-1 flex min-h-0 overflow-hidden relative">
                // Mobile overlays
                <div
                    class=move || {
                        let base_class = "md:hidden absolute inset-0 z-20 bg-black bg-opacity-50 transition-opacity duration-200";
                        if show_threads.get() || show_projects.get() {
                            format!("{base_class} opacity-100")
                        } else {
                            format!("{base_class} opacity-0 pointer-events-none")
                        }
                    }

                    on:click=move |_| {
                        set_show_threads.set(false);
                        set_show_projects.set(false);
                    }
                >
                </div>

                // Threads sidebar
                <div class=move || {
                    if cfg!(feature = "hydrate") {
                        let is_mobile = web_sys::window()
                            .and_then(|w| w.inner_width().ok())
                            .and_then(|w| w.as_f64())
                            .map(|w| w < 768.0)
                            .unwrap_or(false);
                        let base_class = "transition-all duration-200 ease-in-out overflow-hidden border-r border-gray-400 dark:border-teal-700 bg-gray-200 dark:bg-teal-800 flex-shrink-0";
                        if is_mobile {
                            if show_threads.get() {
                                format!(
                                    "{base_class} absolute left-0 top-0 bottom-0 w-80 opacity-100 z-30",
                                )
                            } else {
                                format!(
                                    "{base_class} absolute left-0 top-0 bottom-0 w-0 opacity-0 z-30 pointer-events-none",
                                )
                            }
                        } else {
                            if show_threads.get() {
                                format!("{base_class} w-80 opacity-100")
                            } else {
                                format!("{base_class} w-0 opacity-0")
                            }
                        }
                    } else {
                        let base_class = "transition-all duration-200 ease-in-out overflow-hidden border-r border-gray-400 dark:border-teal-700 bg-gray-200 dark:bg-teal-800 flex-shrink-0";
                        if show_threads.get() {
                            format!("{base_class} w-80 opacity-100")
                        } else {
                            format!("{base_class} w-0 opacity-0")
                        }
                    }
                }>
                    <div class="p-4 h-full overflow-y-auto w-80 dark:border-teal-700 bg-gray-300 dark:bg-teal-900">
                        <ThreadList
                            current_thread_id=thread_id
                            set_current_thread_id=thread_switch_callback
                            set_search_term=set_search_term
                            set_search_action=set_search_action
                            selected_project=selected_project
                            set_selected_project=set_selected_project
                        />
                    </div>
                </div>

                // Main chat area
                <div class="flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
                    <div class="flex-1 overflow-hidden p-2 md:p-4 min-w-0">
                        <div class=move || {
                            let base_class = "h-full max-w-none mx-auto";
                            if show_threads.get() {
                                format!("{base_class} px-0")
                            } else {
                                format!("{base_class} px-0 md:px-56")
                            }
                        }>
                            <MessageList
                                current_thread_id=thread_id
                                set_current_thread_id=set_thread_id
                                refetch_trigger=message_refetch_trigger
                                pending_messages=pending_messages
                                search_term=search_term
                                search_action=search_action
                                title_updates=title_updates
                            />
                        </div>
                    </div>

                    <div class="flex-shrink-0 border-t border-gray-400 dark:border-teal-700 bg-gray-300 dark:bg-teal-900 p-2 md:p-4">
                        <div class=move || {
                            let base_class = "max-w-none mx-auto";
                            if show_threads.get() {
                                format!("{base_class} px-0")
                            } else {
                                format!("{base_class} px-0 md:px-56")
                            }
                        }>
                            <div class="rounded-lg overflow-hidden">
                                <Chat
                                    thread_id=thread_id
                                    on_message_created=on_message_created
                                    pending_messages=set_pending_messages
                                    on_thread_created=on_thread_created
                                />
                            </div>
                        </div>
                    </div>
                </div>

                // Projects sidebar - Desktop: right side, Mobile: overlay
                <div class=move || {
                    if cfg!(feature = "hydrate") {
                        let is_mobile = web_sys::window()
                            .and_then(|w| w.inner_width().ok())
                            .and_then(|w| w.as_f64())
                            .map(|w| w < 768.0)
                            .unwrap_or(false);
                        let base_class = "transition-all duration-200 ease-in-out overflow-hidden border-l border-gray-400 dark:border-teal-700 bg-gray-200 dark:bg-teal-800 flex-shrink-0";
                        if is_mobile {
                            if show_projects.get() {
                                format!(
                                    "{base_class} absolute right-0 top-0 bottom-0 w-80 opacity-100 z-30",
                                )
                            } else {
                                format!(
                                    "{base_class} absolute right-0 top-0 bottom-0 w-0 opacity-0 z-30 pointer-events-none",
                                )
                            }
                        } else {
                            if show_threads.get() {
                                format!("{base_class} w-1/3 opacity-100")
                            } else {
                                format!("{base_class} w-0 opacity-0")
                            }
                        }
                    } else {
                        let base_class = "transition-all duration-200 ease-in-out overflow-hidden border-l border-gray-400 dark:border-teal-700 bg-gray-200 dark:bg-teal-800 flex-shrink-0";
                        if show_threads.get() {
                            format!("{base_class} w-1/3 opacity-100")
                        } else {
                            format!("{base_class} w-0 opacity-0")
                        }
                    }
                }>
                    <div class="p-4 h-full overflow-y-auto w-full">
                        <ProjectsPage
                            selected_project=selected_project
                            set_selected_project=set_selected_project
                        />
                    </div>
                </div>
            </div>

            // floating title notifications - shows completed titles
            <div class="fixed top-20 right-4 z-50 space-y-2 pointer-events-none">
                <For
                    each=move || recent_title_updates.get()
                    key=|notif| notif.id.clone()
                    children=move |notification| {
                        view! {
                            <div class="pointer-events-auto bg-white dark:bg-teal-800 border border-gray-300 dark:border-teal-600 rounded-lg shadow-lg p-3 max-w-sm transform transition-all duration-300 hover:scale-105">
                                <div class="flex items-start gap-2">
                                    <div class="text-seafoam-500 dark:text-mint-400 mt-0.5">
                                        <Icon
                                            icon=icondata_bs::BsLightbulbFill
                                            width="16"
                                            height="16"
                                        />
                                    </div>
                                    <div class="flex-1 min-w-0">
                                        <div class="text-xs text-gray-600 dark:text-gray-400 mb-1">
                                            "✨ Title generated"
                                        </div>
                                        <div class="text-sm font-medium text-gray-800 dark:text-gray-200 break-words">
                                            {notification.title.clone()}
                                        </div>
                                        <div class="text-xs text-gray-500 dark:text-gray-500 mt-1">
                                            "Thread: "
                                            {notification.thread_id.chars().take(8).collect::<String>()}
                                            "..."
                                        </div>
                                    </div>
                                    <button
                                        class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-xs"
                                        on:click=move |_| {
                                            let notif_id = notification.id.clone();
                                            set_recent_title_updates
                                                .update(|notifications| {
                                                    notifications.retain(|n| n.id != notif_id);
                                                });
                                        }
                                    >

                                        "×"
                                    </button>
                                </div>
                            </div>
                        }
                    }
                />

            </div>

            <Toast
                message=toast_message
                visible=toast_visible
                on_close=move || set_toast_visible(false)
            />
        </div>
    }
}

#[derive(Clone, Debug)]
struct TitleNotification {
    id: String,
    thread_id: String,
    title: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[server(CreateThread, "/api")]
pub async fn create_thread() -> Result<String, ServerFnError> {
    use diesel_async::RunQueryDsl; 
    use crate::schema::threads;
    use chrono::Utc;
    use std::fmt;
    use crate::state::AppState;
    use crate::models::conversations::Thread;
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
                ThreadError::Pool(e) => write!(f, "pool error: {e}"),
                ThreadError::Database(e) => write!(f, "database error: {e}"),
                ThreadError::Unauthorized => write!(f, "unauthorized - user not logged in"),
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
        .expect("failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| ThreadError::Pool(e.to_string()))
        .map_err(to_server_error)?;

    let new_thread = Thread {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: Some(Utc::now().naive_utc()),
        updated_at: Some(Utc::now().naive_utc()),
        user_id: Some(user_id),
        parent_thread_id: None,
        branch_point_message_id: None,
        branch_name: None,
        title: None, 
        project_id: None,
    };

    diesel::insert_into(threads::table)
        .values(&new_thread)
        .execute(&mut conn)
        .await
        .map_err(ThreadError::Database)
        .map_err(to_server_error)?;

    Ok(new_thread.id)
}
