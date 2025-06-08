use leptos::{prelude::*, task::spawn_local};

use crate::auth::{verify_token, auth_components::LogoutButton};
use crate::components::dark_mode_toggle::DarkModeToggle;

#[component]
pub fn AuthNav() -> impl IntoView {
    let (is_authenticated, set_is_authenticated) = signal(false);
    let (is_checking, set_is_checking) = signal(true);

    Effect::new(move |_| {
        spawn_local(async move {
            match verify_token().await {
                Ok(is_valid) => {
                    set_is_authenticated(is_valid);
                    set_is_checking(false);
                }
                Err(_) => {
                    set_is_authenticated(false);
                    set_is_checking(false);
                }
            }
        });
    });

    view! {
        <div class="items-end pr-4 flex space-x-4">
            {move || {
                if is_checking() {
                    view! { <span class="text-gray-400">"Loading..."</span> }.into_any()
                } else if is_authenticated() {
                    view! {
                        <>
                            <a
                                href="/admin-panel"
                                class="text-teal-600 dark:text-aqua-400 hover:text-teal-700 dark:hover:text-aqua-300 transition-colors duration-200"
                            >
                                "Admin"
                            </a>
                            <LogoutButton />
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
            </a> <DarkModeToggle />
        </div>
    }
}
