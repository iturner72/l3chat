use leptos::prelude::*;

#[server(InvalidatePoastsCache, "/api")]
pub async fn invalidate_poasts_cache() -> Result<(), ServerFnError> {
    use crate::server_fn::cache::POASTS_CACHE;

    let mut cache = POASTS_CACHE.lock().unwrap();
    *cache = (None, std::time::Instant::now());

    log::info!("Poasts cache invalidated");
    Ok(())
}
