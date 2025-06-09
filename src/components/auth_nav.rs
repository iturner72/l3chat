use leptos::prelude::*;

use crate::auth::{auth_components::LogoutButton, context::AuthContext};
use crate::components::dark_mode_toggle::DarkModeToggle;

#[component]
pub fn AuthNav() -> impl IntoView {
    let auth = use_context::<AuthContext>().expect("AuthContext not found");

    view! {
        <div class="items-end pr-4 flex space-x-4">
            <a
                href="/writersroom"
                class="text-2xl text-teal-600 dark:text-mint-400 hover:text-teal-800 dark:hover:text-mint-300"
            >
                "yap"
            </a>
            {move || {
                if auth.is_loading.get() {
                    view! { <span class="text-gray-400">"Loading..."</span> }.into_any()
                } else if auth.is_authenticated.get() {
                    view! {
                        <>
                            <a
                                href="/admin-panel"
                                class="text-teal-600 dark:text-aqua-400 hover:text-teal-700 dark:hover:text-aqua-300 transition-colors duration-200"
                            >
                                "Admin"
                            </a>
                            <LogoutButton/>
                        </>
                    }
                        .into_any()
                } else {
                    view! {
                        <a
                            href="/admin"
                            class="text-teal-600 dark:text-aqua-400 hover:text-teal-700 dark:hover:text-aqua-300 transition-colors duration-200"
                        >
                            "Login"
                        </a>
                    }
                        .into_any()
                }
            }}

            <a
                href="https://github.com/iturner72/l3chat"
                class="text-teal-600 dark:text-aqua-400 hover:text-teal-700 dark:hover:text-aqua-300 transition-colors duration-200"
                target="_blank"
                rel="noopener noreferrer"
            >
                "github"
            </a>
            <DarkModeToggle/>
        </div>
    }
}
