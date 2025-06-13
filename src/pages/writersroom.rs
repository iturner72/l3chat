use leptos::prelude::*;
use leptos_icons::Icon;
use leptos_fetch::QueryClient;

use crate::components::auth_nav::AuthNav;
use crate::components::chat::Chat;
use crate::components::threadlist::ThreadList;
use crate::components::messagelist::MessageList;
use crate::components::toast::Toast;
use crate::models::conversations::PendingMessage;

async fn create_thread_query() -> Result<String, String> {
    create_thread().await.map_err(|e| e.to_string())
}

#[component]
pub fn WritersRoom() -> impl IntoView {
    let client: QueryClient = expect_context();
    
    let (show_threads, set_show_threads) = signal(false);
    let (thread_id, set_thread_id) = signal("0001".to_string());
    let (toast_visible, set_toast_visible) = signal(false);
    let (toast_message, set_toast_message) = signal(String::new());
    let (message_refetch_trigger, set_message_refetch_trigger) = signal(0);
    let (search_term, set_search_term) = signal(String::new());
    let (search_action, set_search_action) = signal(false);
    let (pending_messages, set_pending_messages) = signal(Vec::<PendingMessage>::new());

    let create_thread_action = Action::new(move |_: &()| {
        async move {
            match create_thread_query().await {
                Ok(new_thread_id) => {
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
                Err(e) => Err(format!("Failed to create thread: {e}"))
            }
        }
    });

    let on_message_created = Callback::new(move |_: ()| {
        set_message_refetch_trigger.update(|n| *n += 1);
    });

    let thread_switch_callback = Callback::new(move |new_id: String| {
        set_thread_id.set(new_id);
        set_message_refetch_trigger.update(|n| *n += 1);
        set_pending_messages.update(|msgs| msgs.clear());
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
            <div class="flex-shrink-0 p-2 border-b border-gray-400 dark:border-teal-700">
                <div class="flex flex-row items-center justify-between">
                    <div class="flex flex-row items-center justify-center space-x-2">
                        <button
                            class="ib text-xs md:text-sm text-gray-900 dark:text-gray-100 hover:text-gray-800 dark:hover:text-gray-200 
                            px-3 py-2 bg-gray-400 dark:bg-teal-700 hover:bg-gray-500 dark:hover:bg-teal-600 
                            border border-gray-600 dark:border-gray-500 hover:border-gray-800 dark:hover:border-gray-400 
                            rounded transition-colors duration-100 flex items-center justify-center"
                            on:click=move |_| set_show_threads.update(|v| *v = !*v)
                        >
                            {move || {
                                if show_threads.get() {
                                    view! {
                                        <Icon
                                            icon=icondata::TbLayoutSidebarRightExpandFilled
                                            width="16"
                                            height="16"
                                        />
                                    }
                                } else {
                                    view! {
                                        <Icon
                                            icon=icondata::TbLayoutSidebarLeftExpandFilled
                                            width="16"
                                            height="16"
                                        />
                                    }
                                }
                            }}

                        </button>
                        <button
                            class="ib text-xs md:text-sm text-teal-700 dark:text-teal-100 hover:text-teal-600 dark:hover:text-teal-200 
                            px-3 py-2 bg-gray-400 dark:bg-teal-700 hover:bg-gray-500 dark:hover:bg-teal-600 
                            border border-gray-600 dark:border-gray-500 hover:border-gray-800 dark:hover:border-gray-400 
                            rounded transition-colors duration-100 flex items-center justify-center"
                            disabled=move || create_thread_action.pending().get()
                            on:click=move |_| {
                                create_thread_action.dispatch(());
                            }
                        >

                            <Icon icon=icondata::FiPlus width="16" height="16"/>
                            {move || {
                                if create_thread_action.pending().get() {
                                    " Creating..."
                                } else {
                                    ""
                                }
                            }}

                        </button>

                        <a
                            href="/draw"
                            class="ib text-xs md:text-sm text-teal-700 dark:text-teal-100 hover:text-teal-600 dark:hover:text-teal-200 
                            px-3 py-2 bg-gray-400 dark:bg-teal-700 hover:bg-gray-500 dark:hover:bg-teal-600 
                            border border-gray-600 dark:border-gray-500 hover:border-gray-800 dark:hover:border-gray-400 
                            rounded transition-colors duration-100 flex items-center justify-center"
                        >
                            <Icon icon=icondata::FaPenFancySolid width="16" height="16"/>
                        </a>

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
                                        <span class="ml-2 text-xs text-mint-600 dark:text-mint-400">
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
                                    <div class="flex items-center px-3 py-2 text-xs text-gray-500 dark:text-gray-400">
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

                    <AuthNav/>
                </div>
            </div>

            <div class="flex-1 flex flex-row min-h-0 overflow-hidden">
                <div class=move || {
                    let base_class = "transition-all duration-100 ease-in-out overflow-hidden border-r border-gray-400 dark:border-teal-700 bg-gray-200 dark:bg-teal-800 flex-shrink-0";
                    if show_threads.get() {
                        format!("{base_class} w-80 opacity-100")
                    } else {
                        format!("{base_class} w-0 opacity-0")
                    }
                }>
                    <div class="p-4 h-full overflow-y-auto w-80">
                        <ThreadList
                            current_thread_id=thread_id
                            set_current_thread_id=thread_switch_callback
                            set_search_term=set_search_term
                            set_search_action=set_search_action
                        />
                    </div>
                </div>

                <div class="flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
                    <div class="flex-1 overflow-hidden p-4 min-w-0">
                        <MessageList
                            current_thread_id=thread_id
                            set_current_thread_id=set_thread_id
                            refetch_trigger=message_refetch_trigger
                            pending_messages=pending_messages
                            search_term=search_term
                            search_action=search_action
                        />
                    </div>

                    <div class="flex-shrink-0 border-t border-gray-400 dark:border-teal-700 bg-gray-100 dark:bg-teal-800 p-4">
                        <Chat
                            thread_id=thread_id
                            on_message_created=on_message_created
                            pending_messages=set_pending_messages
                        />
                    </div>
                </div>
            </div>

            <Toast
                message=toast_message
                visible=toast_visible
                on_close=move || set_toast_visible(false)
            />
        </div>
    }
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
    };

    diesel::insert_into(threads::table)
        .values(&new_thread)
        .execute(&mut conn)
        .await
        .map_err(ThreadError::Database)
        .map_err(to_server_error)?;

    Ok(new_thread.id)
}
