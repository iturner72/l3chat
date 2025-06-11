use leptos::prelude::*;

use crate::components::chat::Chat;
use crate::components::threadlist::{ThreadList, get_threads};
use crate::components::messagelist::MessageList;
use crate::components::toast::Toast;

#[component]
pub fn WritersRoom() -> impl IntoView {
    let (show_threads, set_show_threads) = signal(false);
    let (thread_id, set_thread_id) = signal("0001".to_string());
    let (toast_visible, set_toast_visible) = signal(false);
    let (toast_message, set_toast_message) = signal(String::new());

    let threads = Resource::new(
        || (),
        |_| async move { get_threads().await }
    );

    let create_new_thread = Action::new(move |_: &()| {
        async move {
            match create_thread().await {
                Ok(new_thread_id) => {
                    set_thread_id(new_thread_id.clone());
                    set_toast_message(format!("New thread created: {new_thread_id}"));
                    set_toast_visible(true);

                    threads.refetch();
                    
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

    Effect::new(move |_| {
        if let Some(Err(error)) = create_new_thread.value().get() {
            set_toast_message(error);
            set_toast_visible(true);
            
            set_timeout(
                move || set_toast_visible(false),
                std::time::Duration::from_secs(5)
            );
        }
    });

    view! {
        <div class="w-full flex flex-col bg-gray-300 dark:bg-teal-900 justify-start pt-2 pl-2 pr-2 h-full">
            <div class="flex flex-row items-center justify-between">
                <div class="flex flex-row items-center justify-center space-x-4">
                    <button
                        class="self-start ib text-xs md:text-sm text-gray-900 dark:text-gray-100 hover:text-gray-800 dark:hover:text-gray-200 p-2 border-1 bg-gray-300 dark:bg-teal-700 hover:bg-gray-400 dark:hover:bg-teal-600 border-gray-700 dark:border-gray-600 hover:border-gray-900 dark:hover:border-gray-400"
                        on:click=move |_| set_show_threads.update(|v| *v = !*v)
                    >
                        {move || if show_threads.get() { "←" } else { "→" }}
                    </button>
                    <button
                        class="ib text-xs md:text-sm text-teal-700 dark:text-teal-100 hover:text-teal-600 dark:hover:text-teal-200 p-2 pl-3 pr-3 border-1 bg-gray-300 dark:bg-teal-700 hover:bg-gray-400 dark:hover:bg-teal-600 border-gray-700 dark:border-gray-600 hover:border-gray-900 dark:hover:border-gray-400"
                        on:click=move |_| {
                            create_new_thread.dispatch(());
                        }
                    >

                        "+"

                    </button>
                </div>
            </div>
            <div class="flex flex-row items-start justify-between">
                <div class=move || {
                    let base_class = "transition-all duration-300 ease-in-out overflow-hidden";
                    if show_threads.get() {
                        format!("{base_class} max-w-xs w-full opacity-100")
                    } else {
                        format!("{base_class} max-w-0 w-0 opacity-0")
                    }
                }>
                    <Suspense fallback=move || {
                        view! { <p>"loading threads..."</p> }
                    }>
                        {move || {
                            threads
                                .get()
                                .map(|thread_list| {
                                    match thread_list {
                                        Ok(_threads) => {
                                            view! {
                                                <div>
                                                    <ThreadList
                                                        current_thread_id=thread_id
                                                        set_current_thread_id=set_thread_id
                                                    />
                                                </div>
                                            }
                                                .into_any()
                                        }
                                        Err(e) => {
                                            view! {
                                                <div>{"error loading threads: "} {e.to_string()}</div>
                                            }
                                                .into_any()
                                        }
                                    }
                                })
                        }}

                    </Suspense>
                </div>
                <div class="w-full flex flex-col content-end justify-between h-[calc(80vh-10px)]">
                    <MessageList current_thread_id=thread_id set_current_thread_id=set_thread_id/>
                    <div class="relative text-gray-900 dark:text-gray-100">
                        <Toast
                            message=toast_message
                            visible=toast_visible
                            on_close=move || set_toast_visible(false)
                        />
                        <Chat thread_id=thread_id/>
                    </div>
                </div>
            </div>
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
