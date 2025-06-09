use leptos::prelude::*;
use log::error;
use web_sys::Event;

use crate::models::conversations::ThreadView;

#[component]
pub fn ThreadList(
    current_thread_id: ReadSignal<String>,
    set_current_thread_id: WriteSignal<String>,
    _lab: ReadSignal<String> // will use later
) -> impl IntoView {
    // Use Resource instead of spawn_local for SSR compatibility
    let threads_resource = Resource::new(
        || (), // No dependencies, loads once
        |_| async move { get_threads().await }
    );

    let (search_query, set_search_query) = signal(String::new());
    
    // Resource for search results
    let search_resource = Resource::new(
        move || search_query.get(),
        |query| async move {
            if query.is_empty() {
                get_threads().await
            } else {
                search_threads(query).await
            }
        }
    );

    let handle_search = move |ev: Event| {
        let query = event_target_value(&ev);
        set_search_query.set(query);
    };

    let delete_thread_action = Action::new(move |thread_id: &String| {
        let thread_id = thread_id.clone();
        let current_id = current_thread_id.get();
        async move {
            match delete_thread(thread_id.clone()).await {
                Ok(_) => {
                    // Refetch threads after deletion
                    threads_resource.refetch();
                    search_resource.refetch();

                    // Handle current thread selection
                    if current_id == thread_id {
                        match get_threads().await {
                            Ok(updated_threads) => {
                                if let Some(next_thread) = updated_threads.first() {
                                    set_current_thread_id(next_thread.id.clone());
                                } else {
                                    log::info!("no threads left gang");
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
        <div class="thread-list-container flex flex-col items-start pt-2">
            <input
                type="text"
                placeholder="grep threads!"
                on:input=handle_search
                class="grep-box w-7/12 p-2 mb-2 bg-gray-100 dark:bg-teal-800 text-teal-600 dark:text-mint-400
                border-2 border-gray-300 dark:border-teal-600 focus:border-teal-500 dark:focus:border-mint-300
                focus:outline-none transition duration-300 ease-in-out"
            />

            <Suspense fallback=move || {
                view! { <p>"Loading threads..."</p> }
            }>
                {move || {
                    let resource = if search_query.get().is_empty() {
                        threads_resource.get()
                    } else {
                        search_resource.get()
                    };
                    resource
                        .map(|result| {
                            match result {
                                Ok(thread_list) => {
                                    // Use search_resource if there's a query, otherwise use threads_resource

                                    view! {
                                        <div>
                                            {thread_list
                                                .into_iter()
                                                .map(|thread: ThreadView| {
                                                    let thread_id = thread.id.clone();
                                                    let is_active = current_thread_id() == thread_id;
                                                    let (button_class, text_class) = if is_active {
                                                        (
                                                            "border-teal-500 bg-teal-600 dark:bg-teal-800",
                                                            "text-mint-400 group-hover:text-white dark:text-mint-300 dark:group-hover:text-white",
                                                        )
                                                    } else {
                                                        (
                                                            "border-teal-700 bg-gray-300 dark:bg-teal-800 hover:border-teal-800 hover:bg-gray-900",
                                                            "text-gray-300 group-hover:text-white dark:text-gray-100 dark:group-hover:text-white",
                                                        )
                                                    };
                                                    let thread_id_for_set = thread_id.clone();
                                                    let thread_id_for_delete = thread_id.clone();
                                                    view! {
                                                        <div class="thread-list text-teal-500 dark:text-mint-400 flex flex-col items-start justify-center w-full mb-2">
                                                            <div class="flex w-full justify-between items-center">
                                                                <button
                                                                    class=format!(
                                                                        "thread-item w-full p-2 border-2 {} transition duration-300 ease-in-out group",
                                                                        button_class,
                                                                    )
                                                                    on:click=move |_| set_current_thread_id(
                                                                        thread_id_for_set.clone(),
                                                                    )
                                                                >
                                                                    <p class=format!(
                                                                        "thread-id ib pr-16 md:pr-36 text-base self-start {} transition duration-300 ease-in-out",
                                                                        text_class,
                                                                    )>{thread.id.clone()}</p>
                                                                    <div class="stats-for-nerds hidden group-hover:flex flex-col items-start mt-2">
                                                                        <p class="message-created_at ir text-xs text-teal-300 dark:text-mint-200 group-hover:text-teal-100 dark:group-hover:text-mint-100">
                                                                            created:
                                                                            {thread
                                                                                .created_at
                                                                                .map(|dt| dt.format("%b %d, %I:%M %p").to_string())
                                                                                .unwrap_or_default()}
                                                                        </p>
                                                                    </div>
                                                                </button>
                                                                <button
                                                                    class="delete-button ib text-teal-600 dark:text-mint-400 hover:text-teal-400 dark:hover:text-mint-300 text-sm ml-2 p-2 
                                                                    bg-gray-400 dark:bg-teal-900 hover:bg-gray-500 dark:hover:bg-teal-800 rounded transition duration-300 ease-in-out"
                                                                    on:click=move |_| {
                                                                        delete_thread_action.dispatch(thread_id_for_delete.clone());
                                                                    }
                                                                >
                                                                    "delet"
                                                                </button>
                                                            </div>
                                                        </div>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    }
                                        .into_any()
                                }
                                Err(_e) => {
                                    view! { <div>"Error loading threads: {e}"</div> }.into_any()
                                }
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

#[server(SearchThreads, "/api")]
pub async fn search_threads(query: String) -> Result<Vec<ThreadView>, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::models::conversations::Thread;
    use crate::schema::{threads, messages};
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
        .left_join(messages::table)
        .filter(threads::user_id.eq(other_user_id))
        .filter(
            threads::id.like(format!("%{query}%"))
                .or(messages::content.like(format!("%{query}%")))
        )
        .select(threads::all_columns)
        .distinct()
        .load::<Thread>(&mut conn)
        .await
        .map_err(SearchError::Database)
        .map_err(to_server_error)?;

    Ok(result.into_iter().map(ThreadView::from).collect())
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

    let app_state = use_context::<AppState>()
        .expect("failed to get AppState from context");

    let mut conn = app_state.pool
        .get()
        .await
        .map_err(|e| ThreadError::Pool(e.to_string()))
        .map_err(to_server_error)?;

    conn.transaction(|conn| {
        Box::pin(async move {
            // First, delete all messages associated with thread
            diesel::delete(messages::table.filter(messages::thread_id.eq(&thread_id)))
                .execute(conn)
                .await?;

            // Then, delete thread itself
            diesel::delete(threads::table.find(thread_id))
                .execute(conn)
                .await?;

            Ok(())
        })
    })
    .await
    .map_err(ThreadError::Database)
    .map_err(to_server_error)?;

    Ok(())
}

#[server(GetThreads, "/api")]
pub async fn get_threads() -> Result<Vec<ThreadView>, ServerFnError> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::fmt;

    use crate::state::AppState;
    use crate::models::conversations::Thread;
    use crate::schema::threads::dsl::threads as threads_table;
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

    let result = threads_table
        .filter(crate::schema::threads::user_id.eq(user_id))
        .order(crate::schema::threads::created_at.desc())
        .load::<Thread>(&mut conn)
        .await
        .map_err(ThreadError::Database)
        .map_err(to_server_error)?;

    Ok(result.into_iter().map(ThreadView::from).collect())
}
