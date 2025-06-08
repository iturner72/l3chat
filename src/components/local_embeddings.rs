use leptos::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{EventSource, MessageEvent, ErrorEvent};
use std::collections::HashMap;

use crate::{
    types::StreamResponse,
    server_fn::RssProgressUpdate
};


#[component]
pub fn LocalEmbeddingsProcessor() -> impl IntoView {
    let (progress_states, set_progress_states) = signal::<HashMap<String, RssProgressUpdate>>(HashMap::new());
    let (is_processing, set_is_processing) = signal(false);
    let (no_posts_to_process, set_no_posts_to_process) = signal(false);
    let (current_stream_id, set_current_stream_id) = signal(Option::<String>::None);

    let cancel_embeddings = move || {
        if let Some(stream_id) = current_stream_id.get() {
            let window = web_sys::window().unwrap();
            let url = format!("/api/cancel-stream?stream_id={stream_id}");

            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(_) = JsFuture::from(window.fetch_with_str(&url)).await {
                    log::info!("Stream cancelled");
                }
            });
            set_is_processing(false);
            set_current_stream_id(None);
        }
    };

    let start_embeddings = move || {
        set_is_processing(true);
        set_no_posts_to_process(false);
        set_progress_states.update(|states| states.clear());

        let window = web_sys::window().unwrap();

        wasm_bindgen_futures::spawn_local(async move {
            let resp_value = match JsFuture::from(window.fetch_with_str("/api/create-stream")).await {
                Ok(val) => val,
                Err(e) => {
                    log::error!("Failed to fetch: {e:?}");
                    set_is_processing(false);
                    return;
                }
            };

            let resp = resp_value.dyn_into::<web_sys::Response>().unwrap();
            let json = match JsFuture::from(resp.json().unwrap()).await {
                Ok(json) => json,
                Err(e) => {
                    log::error!("Failed to parse JSON: {e:?}");
                    set_is_processing(false);
                    return;
                }
            };

            let stream_data: StreamResponse = serde_wasm_bindgen::from_value(json).unwrap();
            let stream_id = stream_data.stream_id;

            set_current_stream_id(Some(stream_id.clone()));

            let url = format!("/api/generate-local-embeddings?stream_id={stream_id}");
            let event_source = EventSource::new(&url)
                .expect("Failed to connect to SSE endpoint");

            let event_source_clone = event_source.clone();
            let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
                if let Some(data) = event.data().as_string() {
                    if data == "[DONE]" {
                        event_source_clone.close();
                        set_is_processing(false);
                        set_current_stream_id(None);

                        if progress_states.get().is_empty() {
                            set_no_posts_to_process(true);
                        }
                    } else {
                        match serde_json::from_str::<RssProgressUpdate>(&data) {
                            Ok(update) => {
                                set_progress_states.update(|states| {
                                    states.insert(update.company.clone(), update);
                                });
                            },
                            Err(e) => log::error!("Failed to parse update: {e}")
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);

            let event_source_error = event_source.clone();
            let on_error = Closure::wrap(Box::new(move |error: ErrorEvent| {
                log::error!("SSE Error: {error:?}");
                if let Some(es) = error.target()
                    .and_then(|t| t.dyn_into::<web_sys::EventSource>().ok())
                {
                    if es.ready_state() == web_sys::EventSource::CLOSED {
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().set_href("/admin");
                        }
                    }
                }
                event_source_error.close();
                set_is_processing(false);
                set_current_stream_id(None);
            }) as Box<dyn FnMut(_)>);

            event_source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            event_source.set_onerror(Some(on_error.as_ref().unchecked_ref()));

            on_message.forget();
            on_error.forget();
        });
    };

    view! {
        <div class="p-4 space-y-4">
            <div class="flex items-center justify-between">
                <button
                    class="px-4 py-2 bg-seafoam-500 dark:bg-seafoam-600 text-white rounded 
                    hover:bg-seafoam-400 dark:hover:bg-seafoam-500 transition-colors
                    disabled:bg-gray-400 dark:disabled:bg-gray-600 disabled:cursor-not-allowed"
                    on:click=move |_| {
                        if is_processing.get() { cancel_embeddings() } else { start_embeddings() }
                    }
                >
                    {move || if is_processing() { "Cancel" } else { "Generate Local Embeddings" }}
                </button>

                {move || {
                    is_processing()
                        .then(|| {
                            view! {
                                <span class="text-sm text-seafoam-600 dark:text-seafoam-400">
                                    "Generating embeddings..."
                                </span>
                            }
                        })
                }}
            </div>

            {move || {
                no_posts_to_process()
                    .then(|| {
                        view! {
                            <div class="p-4 bg-gray-100 dark:bg-teal-800 rounded-lg border-l-4 border-mint-500 dark:border-mint-400">
                                <p class="text-gray-700 dark:text-gray-200">
                                    "No posts found that need embeddings! All posts have their embeddings generated."
                                </p>
                            </div>
                        }
                    })
            }}

            {move || {
                let states = progress_states.get();
                if !states.is_empty() {
                    view! {
                        <div class="grid gap-3">
                            {states
                                .values()
                                .map(|update| {
                                    let is_completed = update.status == "completed";
                                    let status_class = if is_completed {
                                        "bg-seafoam-100 dark:bg-seafoam-900 text-seafoam-800 dark:text-seafoam-200"
                                    } else {
                                        "bg-aqua-100 dark:bg-aqua-900 text-aqua-800 dark:text-aqua-200"
                                    };
                                    let border_class = if is_completed {
                                        "border-seafoam-500 dark:border-mint-400"
                                    } else {
                                        "border-aqua-500 dark:border-aqua-400"
                                    };

                                    view! {
                                        <div class=format!(
                                            "p-4 rounded-lg border-l-4 bg-gray-100 dark:bg-teal-800 {}",
                                            border_class,
                                        )>
                                            <div class="flex justify-between items-center mb-2">
                                                <span class="font-medium text-gray-800 dark:text-gray-200">
                                                    {update.company.clone()}
                                                </span>
                                                <span class=format!(
                                                    "px-2 py-1 text-sm rounded {}",
                                                    status_class,
                                                )>{update.status.clone()}</span>
                                            </div>

                                            <div class="space-y-2 text-sm">
                                                <div class="grid grid-cols-2 text-gray-600 dark:text-gray-300">
                                                    <span>"Processed"</span>
                                                    <span class="text-right">{update.new_posts}</span>
                                                </div>
                                                <div class="grid grid-cols-2 text-gray-600 dark:text-gray-300">
                                                    <span>"Failed"</span>
                                                    <span class="text-right">{update.skipped_posts}</span>
                                                </div>
                                                {update
                                                    .current_post
                                                    .as_ref()
                                                    .map(|post| {
                                                        let post = post.clone();
                                                        view! {
                                                            <div class="mt-2">
                                                                <span class="text-gray-500 dark:text-gray-400">
                                                                    "Current: "
                                                                </span>
                                                                <span class="text-gray-700 dark:text-gray-200 line-clamp-1">
                                                                    {move || post.clone()}
                                                                </span>
                                                            </div>
                                                        }
                                                    })}
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}
        </div>
    }
}
