pub mod app;
pub mod auth;
#[cfg(feature = "ssr")]
pub mod cancellable_sse;
pub mod components;
pub mod database;
pub mod embedding_service;
pub mod embeddings_service;
pub mod handlers;
pub mod models;
pub mod pages;
#[cfg(feature = "ssr")]
pub mod schema;
pub mod server_fn;
#[cfg(feature = "ssr")]
pub mod services;
pub mod state;
pub mod supabase;
pub mod types;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
