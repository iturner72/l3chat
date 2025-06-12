use leptos::prelude::*;
use log::error;
use web_sys::Event;

use crate::models::conversations::ThreadView;

#[component]
pub fn ThreadList(
    current_thread_id: ReadSignal<String>,
    #[prop(into)] set_current_thread_id: Callback<String>,
) -> impl IntoView {
    // Use Resource instead of spawn_local for SSR compatibility
    let threads_resource = Resource::new(
        || (), // No dependencies, loads once
        |_| async move { get_threads().await }
    );

    let (search_query, set_search_query) = signal(String::new());
    
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
        let current_id = current_thread_id.get_untracked(); 
        async move {
            match delete_thread(thread_id.clone()).await {
                Ok(_) => {
                    threads_resource.refetch();
                    search_resource.refetch();

                    if current_id == thread_id {
                        match get_threads().await {
                            Ok(updated_threads) => {
                                if let Some(next_thread) = updated_threads.first() {
                                    set_current_thread_id.run(next_thread.id.clone());
                                } else {
                                    log::info!("no threads left gang");
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

    // Helper function to build thread tree structure - make it deterministic
    let build_thread_tree = |threads: Vec<ThreadView>| -> Vec<ThreadNode> {
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
    };

    view! {
        <div class="thread-list-container flex flex-col items-start pt-2">
            <div class="relative flex items-center w-7/12">
                <input
                    type="text"
                    placeholder="grep threads"
                    on:input=handle_search
                    class="grep-box w-full pr-10 p-2 mb-2 bg-gray-100 dark:bg-teal-800 text-teal-600 dark:text-mint-400
                    border-0 border-gray-300 dark:border-teal-600 focus:border-teal-500 dark:focus:border-mint-300
                    focus:outline-none transition duration-0 ease-in-out"
                />
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    class="h-5 w-5 absolute right-3 text-gray-400 dark:text-teal-500"
                    viewBox="0 1 20 20"
                    fill="currentColor"
                >
                    <path
                        fill-rule="evenodd"
                        d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z"
                        clip-rule="evenodd"
                    ></path>
                </svg>
            </div>

            <Suspense fallback=move || {
                view! {
                    <div class="w-7/12">
                        <p class="text-gray-500 dark:text-gray-400 text-sm">"Loading threads..."</p>
                    </div>
                }
            }>
                <For
                    each=move || {
                        let resource = if !search_query.get().is_empty() {
                            search_resource.get()
                        } else {
                            threads_resource.get()
                        };
                        resource
                            .map(|result| {
                                match result {
                                    Ok(thread_list) => {
                                        if thread_list.is_empty() {
                                            Vec::new()
                                        } else {
                                            build_thread_tree(thread_list)
                                        }
                                    }
                                    Err(_) => Vec::new(),
                                }
                            })
                            .unwrap_or_default()
                    }

                    key=|root_node| root_node.thread.id.clone()
                    children=move |root_node| {
                        view! {
                            <ThreadTreeNode
                                node=root_node
                                current_thread_id=current_thread_id
                                set_current_thread_id=set_current_thread_id
                                delete_action=delete_thread_action
                                depth=0
                            />
                        }
                    }
                />

                {move || {
                    let resource = if !search_query.get().is_empty() {
                        search_resource.get()
                    } else {
                        threads_resource.get()
                    };
                    if let Some(Ok(thread_list)) = resource {
                        if thread_list.is_empty() {
                            view! {
                                <div class="w-7/12">
                                    <p class="text-gray-500 dark:text-gray-400 text-sm">
                                        "No threads found"
                                    </p>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    } else if let Some(Err(e)) = resource {
                        view! {
                            <div class="w-7/12">
                                <div class="text-red-500 text-sm">
                                    "Error loading threads: " {e.to_string()}
                                </div>
                            </div>
                        }
                            .into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}

            </Suspense>
        </div>
    }
}

#[derive(Debug, Clone)]
struct ThreadNode {
    thread: ThreadView,
    children: Vec<ThreadNode>,
}

#[component]
fn ThreadTreeNode(
    node: ThreadNode,
    current_thread_id: ReadSignal<String>,
    #[prop(into)] set_current_thread_id: Callback<String>,
    delete_action: Action<String, ()>,
    depth: usize,
    #[prop(optional)] is_last_child: bool,
) -> impl IntoView {
    let thread = node.thread.clone();
    let thread_id = thread.id.clone();
    let thread_id_for_memo = thread_id.clone();
    let thread_id_for_set = thread_id.clone();
    let thread_id_for_delete = thread_id.clone();
    let thread_for_display = thread.clone();
    let thread_for_styles = thread.clone();
    
    // Calculate indentation based on depth
    let margin_left = format!("{}rem", depth as f32 * 1.5);
    
    // Use a memo for the active state to make it reactive properly
    let is_active = Memo::new(move |_| current_thread_id.get() == thread_id_for_memo);
    
    // Determine styling based on whether it's a main thread or branch AND if it's active
    let get_styles = move || {
        let active = is_active.get();
        let is_branch = thread_for_styles.parent_thread_id.is_some();
        
        if is_branch {
            // This is a branch - always use branch emoji
            if active {
                (
                    "🌿",
                    "border-seafoam-500 bg-seafoam-600 dark:bg-seafoam-700",
                    "text-white group-hover:text-white",
                )
            } else {
                (
                    "🌿",
                    "border-gray-600 bg-gray-200 dark:bg-teal-700 hover:border-seafoam-600 hover:bg-gray-300 dark:hover:bg-teal-600",
                    "text-gray-600 group-hover:text-gray-800 dark:text-gray-300 dark:group-hover:text-white",
                )
            }
        } else {
            // This is a main thread - use thread emoji
            if active {
                (
                    "🧵",
                    "border-teal-500 bg-teal-600 dark:bg-teal-700",
                    "text-white group-hover:text-white",
                )
            } else {
                (
                    "🧵",
                    "border-teal-700 bg-gray-300 dark:bg-teal-800 hover:border-teal-800 hover:bg-gray-400 dark:hover:bg-gray-700",
                    "text-gray-700 group-hover:text-white dark:text-gray-100 dark:group-hover:text-white",
                )
            }
        }
    };

    let get_display_name = move |thread: &ThreadView| {
        if let Some(branch_name) = &thread.branch_name {
            // For branches, just show the branch emoji and number
            format!("branch {}", branch_name)
        } else if thread.parent_thread_id.is_some() {
            // Fallback for branches without explicit names
            "branch".to_string()
        } else {
            // For main threads, show truncated thread ID
            if thread.id.len() > 8 {
                format!("{}...", &thread.id[..8])
            } else {
                thread.id.clone()
            }
        }
    };

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
                        view! {
                            <button
                                class=format!(
                                    "thread-item w-full p-2 border-0 {} rounded-md transition duration-0 ease-in-out group text-sm relative",
                                    button_class,
                                )

                                on:click=move |_| {
                                    log::info!("Clicked thread: {}", thread_id_for_click);
                                    set_current_thread_id.run(thread_id_for_click.clone());
                                }
                            >

                                <div class="flex items-center">
                                    <span class="mr-2">{icon}</span>
                                    <p class=format!(
                                        "thread-name ib text-sm {} transition duration-0 ease-in-out",
                                        text_class,
                                    )>{get_display_name(&thread_for_display)}</p>
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
                                />
                            }
                        }
                    />

                </div>
            </div>
        </div>
    }.into_any()
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
        .left_join(messages::table.on(messages::thread_id.eq(threads::id)))
        .filter(threads::user_id.eq(other_user_id))
        .filter(
            threads::id.ilike(format!("%{query}%"))
                .or(messages::content.ilike(format!("%{query}%")))
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

#[server(CreateBranch, "/api")]
pub async fn create_branch(
    source_thread_id: String,
    branch_point_message_id: i32,
    _branch_name: Option<String>, // Optional parameter for custom naming
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

    // Helper function to reconstruct conversation history recursively
    fn get_full_conversation_history<'a>(
        conn: &'a mut diesel_async::AsyncPgConnection,
        thread_id: &'a str,
        branch_point_message_id: i32,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Message>, diesel::result::Error>> + Send + 'a>> {
        Box::pin(async move {
            // Get the source thread
            let source_thread = threads::table
                .find(thread_id)
                .first::<Thread>(conn)
                .await?;

            let mut all_messages = Vec::new();

            // If this thread has a parent, get the conversation history from the parent first
            if let Some(parent_thread_id) = &source_thread.parent_thread_id {
                if let Some(parent_branch_point) = source_thread.branch_point_message_id {
                    // Recursively get messages from parent up to its branch point
                    let parent_messages = get_full_conversation_history(
                        conn, 
                        parent_thread_id, 
                        parent_branch_point
                    ).await?;
                    all_messages.extend(parent_messages);
                }
            }

            // Get messages from the current thread
            let current_messages = if source_thread.parent_thread_id.is_some() {
                // If this is a branch, get messages from this thread up to (but not including) the branch point
                messages::table
                    .filter(messages::thread_id.eq(thread_id))
                    .filter(messages::id.lt(branch_point_message_id))
                    .order(messages::id.asc())
                    .load::<Message>(conn)
                    .await?
            } else {
                // If this is a root thread, get messages up to (but not including) the branch point
                messages::table
                    .filter(messages::thread_id.eq(thread_id))
                    .filter(messages::id.lt(branch_point_message_id))
                    .order(messages::id.asc())
                    .load::<Message>(conn)
                    .await?
            };

            all_messages.extend(current_messages);
            Ok(all_messages)
        })
    }

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

            // Get the full conversation history up to the branch point
            let messages_to_copy = get_full_conversation_history(
                conn,
                &source_thread_id_clone,
                branch_point_message_id
            ).await?;

            if messages_to_copy.is_empty() {
                return Err(BranchError::NotFound);
            }

            // Create new thread
            let new_thread = Thread {
                id: new_thread_id_clone.clone(),
                created_at: Some(chrono::Utc::now().naive_utc()),
                updated_at: Some(chrono::Utc::now().naive_utc()),
                user_id: Some(user_id),
                parent_thread_id: Some(source_thread_id_clone.clone()),
                branch_point_message_id: Some(branch_point_message_id),
                branch_name: Some(branch_name),
            };

            diesel::insert_into(threads::table)
                .values(&new_thread)
                .execute(conn)
                .await?;

            // Copy messages to new thread with new IDs
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

            Ok(new_thread_id_clone)
        })
    })
    .await?;

    log::info!("Created branch {} from thread {} at message {}", result, source_thread_id, branch_point_message_id);
    Ok(result)
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
