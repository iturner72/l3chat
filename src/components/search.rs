use leptos::leptos_dom::helpers::TimeoutHandle;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, PartialEq)]
pub struct SearchParams {
    pub query: String,
    pub search_type: SearchType,
}

#[derive(Copy, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SearchType {
    Basic,
    OpenAISemantic,
    LocalSemantic,
}

impl SearchType {
    fn placeholder(&self) -> &'static str {
        match self {
            SearchType::Basic => "Search blog posts...",
            SearchType::OpenAISemantic => "Search blog posts using OpenAI...",
            SearchType::LocalSemantic => "Search blog posts using local (all-MiniLM-L6-v2) AI...",
        }
    }
}

#[component]
pub fn BlogSearch(#[prop(into)] on_search: Callback<SearchParams>) -> impl IntoView {
    let (search_term, set_search_term) = signal(String::new());
    let (search_type, set_search_type) = signal(SearchType::Basic);
    let timeout_handle: StoredValue<Option<TimeoutHandle>> = StoredValue::new(None);

    let handle_search = move |current: String, stype: SearchType| {
        on_search.run(SearchParams {
            query: current,
            search_type: stype,
        });
    };

    // create debounced effect for search
    Effect::new(move |_| {
        let current = search_term.get();
        let current_type = search_type.get();

        if let Some(handle) = timeout_handle.get_value() {
            handle.clear();
        }

        let handle = set_timeout_with_handle(
            move || {
                handle_search(current, current_type);
            },
            Duration::from_millis(500),
        )
        .expect("Failed to set timeout");

        timeout_handle.set_value(Some(handle));
    });

    let clear_search = move |_| {
        on_search.run(SearchParams {
            query: String::new(),
            search_type: search_type.get(),
        });
        set_search_term(String::new());
    };

    let select_class = "px-3 py-1.5 text-sm rounded-md transition-colors border-2 \
                       bg-white dark:bg-teal-800 text-gray-700 dark:text-gray-200 \
                       border-teal-600 dark:border-seafoam-600 \
                       focus:border-seafoam-500 dark:focus:border-aqua-400 \
                       hover:border-seafoam-500 dark:hover:border-aqua-400";

    view! {
        <div class="w-full max-w-2xl mx-auto mb-6">
            <div class="flex items-center gap-4 mb-2">
                <select
                    class=select_class
                    on:change=move |ev| {
                        let value = event_target_value(&ev);
                        let new_type = match value.as_str() {
                            "openai" => SearchType::OpenAISemantic,
                            "local" => SearchType::LocalSemantic,
                            _ => SearchType::Basic,
                        };
                        set_search_type.set(new_type);
                        handle_search(search_term.get(), new_type);
                    }
                    prop:value=move || match search_type.get() {
                        SearchType::Basic => "basic",
                        SearchType::OpenAISemantic => "openai",
                        SearchType::LocalSemantic => "local",
                    }
                >
                    <option value="basic">"Basic Search"</option>
                    <option value="openai">"OpenAI Semantic Search"</option>
                    <option value="local">"all-MiniLM-L6-v2 Semantic Search"</option>
                </select>
            </div>
            <div class="relative">
                <input
                    type="text"
                    placeholder=move || search_type.get().placeholder()
                    prop:value=search_term
                    on:input=move |ev| {
                        set_search_term(event_target_value(&ev));
                    }
                    class="w-full px-4 py-2 text-gray-800 dark:text-gray-200
                    bg-white dark:bg-teal-800 
                    border-2 border-teal-600 dark:border-seafoam-600
                    focus:border-seafoam-500 dark:focus:border-aqua-500 
                    rounded-lg shadow-sm
                    focus:outline-none transition duration-0"
                />
                {move || {
                    (!search_term.get().is_empty())
                        .then(|| {
                            view! {
                                <button
                                    on:click=clear_search
                                    class="absolute right-3 top-1/2 -translate-y-1/2
                                    text-gray-400 hover:text-gray-600 
                                    dark:text-gray-500 dark:hover:text-gray-300"
                                >
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        class="h-5 w-5"
                                        viewBox="0 0 20 20"
                                        fill="currentColor"
                                    >
                                        <path
                                            fill-rule="evenodd"
                                            d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z"
                                            clip-rule="evenodd"
                                        />
                                    </svg>
                                </button>
                            }
                        })
                }}
            </div>
        </div>
    }
}
