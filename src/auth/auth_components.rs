use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::auth::{context::AuthContext, get_current_user, Logout};
use crate::components::embeddings::EmbeddingsProcessor;
use crate::components::local_embeddings::LocalEmbeddingsProcessor;

#[component]
pub fn AdminLogin() -> impl IntoView {
    view! {
        <div class="min-h-screen bg-gray-100 dark:bg-teal-900 flex items-center justify-center">
            <div class="max-w-md w-full bg-white dark:bg-teal-800 rounded-lg shadow-md p-6">
                <h2 class="text-2xl font-bold text-center text-gray-800 dark:text-gray-200 mb-6">
                    "Admin Login"
                </h2>

                <div class="space-y-4">
                    <a
                        href="/auth/google"
                        target="_self"
                        class="w-full flex items-center justify-center px-4 py-2
                        bg-white dark:bg-teal-700 border border-gray-300 dark:border-teal-600 
                        rounded-md shadow-sm text-gray-700 dark:text-gray-200 
                        hover:bg-gray-50 dark:hover:bg-teal-600 transition-colors"
                    >
                        <svg class="w-5 h-5 mr-2" viewBox="0 0 24 24">
                            <path
                                fill="#4285F4"
                                d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                            ></path>
                            <path
                                fill="#34A853"
                                d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                            ></path>
                            <path
                                fill="#FBBC05"
                                d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                            ></path>
                            <path
                                fill="#EA4335"
                                d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                            ></path>
                        </svg>
                        "Continue with Google"
                    </a>

                    <a
                        href="/auth/discord"
                        target="_self"
                        class="w-full flex items-center justify-center px-4 py-2
                        bg-indigo-600 dark:bg-indigo-700 border border-indigo-600 dark:border-indigo-700 
                        rounded-md shadow-sm text-white 
                        hover:bg-indigo-700 dark:hover:bg-indigo-600 transition-colors"
                    >
                        <svg class="w-5 h-5 mr-2" fill="currentColor" viewBox="0 0 24 24">
                            <path d="M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419-.0002 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9554 2.4189-2.1568 2.4189Z"></path>
                        </svg>
                        "Continue with Discord"
                    </a>
                </div>

                <div class="mt-6 text-center">
                    <a
                        href="/"
                        class="text-sm text-seafoam-600 dark:text-aqua-400 hover:text-seafoam-700 dark:hover:text-aqua-300"
                    >
                        "← Back to Home"
                    </a>
                </div>
            </div>
        </div>
    }.into_any()
}

#[component]
pub fn ProtectedAdminPanel() -> impl IntoView {
    let current_user = Resource::new(|| (), |_| get_current_user());

    view! {
        <Suspense fallback=|| {
            view! { <div class="p-4">"Loading..."</div> }.into_any()
        }>
            {move || {
                current_user
                    .get()
                    .map(|user_result| {
                        match user_result {
                            Ok(Some(user)) => {
                                view! {
                                    <div class="min-h-screen bg-gray-100 dark:bg-teal-900">
                                        <div class="flex justify-between items-center p-4 border-b border-gray-200 dark:border-teal-700">
                                            <div class="flex items-center space-x-4">
                                                <a
                                                    href="/"
                                                    class="text-3xl text-seafoam-600 dark:text-mint-400 font-bold"
                                                >
                                                    "l3chat"
                                                </a>
                                                <span class="text-gray-600 dark:text-gray-300">
                                                    "Admin Panel"
                                                </span>
                                            </div>
                                            <div class="flex items-center space-x-4">
                                                {user
                                                    .avatar_url
                                                    .as_ref()
                                                    .map(|avatar| {
                                                        view! {
                                                            <img
                                                                src=avatar.clone()
                                                                alt="User avatar"
                                                                class="w-8 h-8 rounded-full"
                                                            />
                                                        }
                                                            .into_any()
                                                    })}
                                                <span class="text-gray-700 dark:text-gray-200">
                                                    {user
                                                        .display_name
                                                        .clone()
                                                        .or(user.username.clone())
                                                        .unwrap_or_else(|| "Anonymous".to_string())}
                                                </span> <LogoutButton/>
                                            </div>
                                        </div>

                                        <div class="container mx-auto p-6">
                                            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                                                <div class="bg-white dark:bg-teal-800 rounded-lg shadow-md">
                                                    <div class="p-4 border-b border-gray-200 dark:border-teal-700">
                                                        <h2 class="text-xl font-semibold text-gray-800 dark:text-gray-200">
                                                            "OpenAI Embeddings"
                                                        </h2>
                                                    </div>
                                                    <EmbeddingsProcessor/>
                                                </div>

                                                <div class="bg-white dark:bg-teal-800 rounded-lg shadow-md">
                                                    <div class="p-4 border-b border-gray-200 dark:border-teal-700">
                                                        <h2 class="text-xl font-semibold text-gray-800 dark:text-gray-200">
                                                            "Local Embeddings"
                                                        </h2>
                                                    </div>
                                                    <LocalEmbeddingsProcessor/>
                                                </div>
                                            </div>

                                            <div class="mt-6 bg-white dark:bg-teal-800 rounded-lg shadow-md p-6">
                                                <h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200 mb-4">
                                                    "User Information"
                                                </h3>
                                                <div class="grid grid-cols-2 gap-4 text-sm">
                                                    <div>
                                                        <span class="font-medium text-gray-600 dark:text-gray-400">
                                                            "Provider:"
                                                        </span>
                                                        <span class="ml-2 text-gray-800 dark:text-gray-200">
                                                            {user.provider}
                                                        </span>
                                                    </div>
                                                    <div>
                                                        <span class="font-medium text-gray-600 dark:text-gray-400">
                                                            "Email:"
                                                        </span>
                                                        <span class="ml-2 text-gray-800 dark:text-gray-200">
                                                            {user.email.unwrap_or_else(|| "N/A".to_string())}
                                                        </span>
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                            Ok(None) => {
                                view! {
                                    <div class="min-h-screen bg-gray-100 dark:bg-teal-900 flex items-center justify-center">
                                        <div class="text-center">
                                            <h2 class="text-2xl font-bold text-gray-800 dark:text-gray-200 mb-4">
                                                "Access Denied"
                                            </h2>
                                            <p class="text-gray-600 dark:text-gray-400 mb-6">
                                                "You need to log in to access the admin panel."
                                            </p>
                                            <a
                                                href="/admin"
                                                class="px-4 py-2 bg-seafoam-600 dark:bg-teal-600 text-white rounded-md hover:bg-seafoam-700 dark:hover:bg-teal-700"
                                            >
                                                "Go to Login"
                                            </a>
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(_) => {
                                view! {
                                    <div class="min-h-screen bg-gray-100 dark:bg-teal-900 flex items-center justify-center">
                                        <div class="text-center">
                                            <h2 class="text-2xl font-bold text-salmon-600 mb-4">
                                                "Error"
                                            </h2>
                                            <p class="text-gray-600 dark:text-gray-400">
                                                "Failed to load user information."
                                            </p>
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                    })
            }}

        </Suspense>
    }.into_any()
}

#[component]
pub fn LogoutButton() -> impl IntoView {
    let logout_action = ServerAction::<Logout>::new();
    let navigate = use_navigate();
    let auth = use_context::<AuthContext>().expect("AuthContext not found");

    Effect::new(move |_| {
        if logout_action.value().get().is_some() {
            auth.refresh_auth();
            navigate("/", Default::default());
        }
    });

    view! {
        <button
            on:click=move |_| {
                logout_action.dispatch(Logout {});
            }

            class="px-3 py-1 text-sm bg-salmon-600 hover:bg-salmon-700 text-gray-600 dark:text-gray-400 rounded-md transition-colors"
        >
            "Logout"
        </button>
    }.into_any()
}
