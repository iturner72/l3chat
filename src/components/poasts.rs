use leptos::prelude::*;
use serde::{Serialize, Deserialize};
use std::borrow::Cow;

use crate::components::search::{BlogSearch, SearchParams, SearchType};

#[cfg(feature = "hydrate")]
macro_rules! console_log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into());
    };
}

#[cfg(not(feature = "hydrate"))]
macro_rules! console_log {
    ($($t:tt)*) => {
        log::info!($($t)*);
    };
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Poast {
    pub id: i32,
    pub published_at: String,
    pub company: String,
    pub title: String,
    pub link: String,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub full_text: Option<String>,
    pub links: Option<Links>,
    pub similarity: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Links {
    pub logo_url: Option<String>,
}

#[server(GetCompanies, "/api")]
pub async fn get_companies() -> Result<Vec<String>, ServerFnError> {
    use crate::supabase::get_client;
    use log::{debug, error};
    use std::fmt;

    #[derive(Debug)]
    enum CompaniesError {
        Request(String),
        Parse(String),
    }
    
    impl fmt::Display for CompaniesError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                CompaniesError::Request(e) => write!(f, "reqwest error: {e}"),
                CompaniesError::Parse(e) => write!(f, "JSON parse error: {e}"),
            }
        }
    }
    
    fn to_server_error(e: CompaniesError) -> ServerFnError {
        ServerFnError::ServerError(e.to_string())
    }


    let client = get_client();

    let response = client
        .from("links")
        .select("company")
        .execute()
        .await
        .map_err(|e| {
            error!("Failed to fetch companies: {e}");
            CompaniesError::Request("Failed to fetch companies".to_string())
        }).map_err(to_server_error)?;

    let response_text = response.text().await
        .map_err(|e| {
            error!("Failed to get response text: {e}");
            CompaniesError::Request("Failed to read response".to_string())
        }).map_err(to_server_error)?;

    let companies: Vec<serde_json::Value> = serde_json::from_str(&response_text)
        .map_err(|e| {
            error!("Failed to parse JSON: {e}");
            CompaniesError::Parse("Failed to parse companies data".to_string())
        }).map_err(to_server_error)?;

    let mut company_names: Vec<String> = companies
        .into_iter()
        .filter_map(|v| v["company"].as_str().map(String::from))
        .collect();
    company_names.sort();
    company_names.dedup();

    debug!("Successfully fetched {} companies", company_names.len());
    Ok(company_names)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostFilter {
    pub search_term: Option<String>,
    pub company: Option<String>,
}

#[server(GetPoasts, "/api")]
pub async fn get_poasts(filter: Option<PostFilter>) -> Result<Vec<Poast>, ServerFnError> {
    use crate::supabase::get_client;
    use serde_json::from_str;
    use std::fmt;
    use log::{debug, info, error};
    use std::time::Instant;
    use crate::server_fn::cache::{POASTS_CACHE, CACHE_DURATION};

    #[derive(Debug)]
    enum PoastError {
        RequestError(String),
        JsonParseError(String),
    }
    
    impl fmt::Display for PoastError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                PoastError::RequestError(e) => write!(f, "reqwest error: {e}"),
                PoastError::JsonParseError(e) => write!(f, "JSON parse error: {e}"),
            }
        }
    }
    
    fn to_server_error(e: PoastError) -> ServerFnError {
        ServerFnError::ServerError(e.to_string())
    }

    // check cache only if no search term is provided nor company filter is active
    if let Some(filter) = &filter {
        if filter.search_term.is_none() && filter.company.is_none() {
            let cache_duration = CACHE_DURATION;
            let cached_data = POASTS_CACHE.lock().unwrap().clone();

            if let (Some(cached_poasts), last_fetch) = cached_data {
                if last_fetch.elapsed() < cache_duration {
                    info!("Returning cached poasts");
                    info!("Cache debug: {:?}", (cached_poasts.len(), last_fetch));
                    return Ok(cached_poasts);
                }
            }
        }
    } else {
        // No filter at all, definitely use cache
        let cache_duration = CACHE_DURATION;
        let cached_data = POASTS_CACHE.lock().unwrap().clone();

        if let (Some(cached_poasts), last_fetch) = cached_data {
            if last_fetch.elapsed() < cache_duration {
                info!("Returning cached poasts");
                info!("Cache debug: {:?}", (cached_poasts.len(), last_fetch));
                return Ok(cached_poasts);
            }
        }
    }

    info!("fetching blog poasts from supabase...");
    let client = get_client();

    let mut request = client
        .from("poasts")
        .select("id, published_at, company, title, link, summary, links!posts_company_fkey(logo_url)")
        .order("published_at.desc")
        .limit(30);

    if let Some(ref filter) = filter {
        if let Some(ref term) = filter.search_term {
            if !term.trim().is_empty() {
                debug!("Searching for term: {term}");
                request = request.or(format!(
                    "title.ilike.%{term}%,summary.ilike.%{term}%"
                ));
            }
        }

        if let Some(ref company) = filter.company.clone().filter(|c| !c.trim().is_empty()) {
            debug!("Filtering by company: {company}");
            request = request.eq("company", company);
        } else {
            debug!("No company filter applied - showing all companies");
        }
    }

    let response = request
        .execute()
        .await
        .map_err(|e| {
            error!("supabase request error: {e}");
            PoastError::RequestError(e.to_string())
        }).map_err(to_server_error)?;

    debug!("received response from Supabase");
    debug!("response status: {:?}", response.status());
    
    let body = response.text().await.map_err(|e| {
        error!("error reading response body: {e}");
        PoastError::RequestError(e.to_string())
    }).map_err(to_server_error)?;

    debug!("response body length: {}", body.len());

    if body.trim().is_empty() {
        error!("empty response from Supabase");
        return Err(ServerFnError::ServerError("empty response from Supabase".to_string()));
    }

    let poasts: Vec<Poast> = from_str(&body).map_err(|e| {
        error!("JSON parse error: {e}. Body: {body}");
        PoastError::JsonParseError(format!("failed to parse JSON: {e}"))
    }).map_err(to_server_error)?;

    info!("successfully parsed {} poasts", poasts.len());

    // update cache
    if let Some(filter) = &filter {
        if filter.search_term.is_none() && filter.company.is_none(){ 
            let mut cache = POASTS_CACHE.lock().unwrap();
            *cache = (Some(poasts.clone()), Instant::now());
        }
    } else {
        let mut cache = POASTS_CACHE.lock().unwrap();
        *cache = (Some(poasts.clone()), Instant::now());
    }

    Ok(poasts)
}

#[component]
pub fn Poasts() -> impl IntoView {
    let (search_params, set_search_params) = signal(SearchParams {
        query: String::new(),
        search_type: SearchType::Basic
    });
    let (selected_company, set_selected_company) = signal(String::new());

    let companies = Resource::new(|| (), |_| get_companies());

    let poasts = Resource::new(
        move || {
            let params = search_params.get();
            let company = selected_company.get();

            console_log!(
                "Filter changed - search: '{}', type: {:?}, company: '{}'", 
                params.query, 
                params.search_type,
                company
            );
            
            (params, company)
        },
        move |(params, company)| async move {
            match params.search_type {
                SearchType::OpenAISemantic | SearchType::LocalSemantic => {
                    if params.query.trim().is_empty() {
                        get_poasts(None).await
                    } else {
                        let semantic_results = semantic_search(params.query, params.search_type).await?;
                        if company.trim().is_empty() {
                            Ok(semantic_results)
                        } else {
                            Ok(semantic_results
                                .into_iter()
                                .filter(|post| post.company == company)
                                .collect())
                        }
                    }
                }
                SearchType::Basic => {
                    let filter = PostFilter {
                        search_term: if params.query.trim().is_empty() { None } else { Some(params.query) },
                        company: if company.trim().is_empty() { None } else { Some(company) },
                    };
                    get_poasts(Some(filter)).await
                }
            }
        }
    );

    let on_search = Callback::new(move |new_params: SearchParams| {
        set_search_params.set(new_params);
    });

    view! {
        <div class="pt-4 space-y-4">
            <BlogSearch on_search=on_search />

            <Suspense fallback=|| view! { <div class="pl-4 h-10"></div> }>
                <div class="flex justify-start mb-2 pl-4">
                    {move || {
                        companies
                            .get()
                            .map(|companies_result| {
                                let selected = selected_company.get();
                                match companies_result {
                                    Ok(companies) => {
                                        view! {
                                            <>
                                                <select
                                                    on:change=move |ev| set_selected_company(
                                                        event_target_value(&ev),
                                                    )
                                                    class="w-52 p-2 rounded-md bg-gray-100 dark:bg-teal-800 text-gray-800 dark:text-gray-200 
                                                    border border-teal-500 dark:border-seafoam-500 
                                                    focus:border-seafoam-600 dark:focus:border-aqua-400 
                                                    focus:outline-none focus:ring-2 focus:ring-seafoam-500 dark:focus:ring-aqua-400"
                                                >
                                                    <option value="">"All Companies"</option>
                                                    {companies
                                                        .into_iter()
                                                        .map(|company| {
                                                            view! {
                                                                <option value=company.clone() selected=selected == company>
                                                                    {company.clone()}
                                                                </option>
                                                            }
                                                        })
                                                        .collect_view()}
                                                </select>
                                            </>
                                        }
                                            .into_any()
                                    }
                                    Err(_) => {
                                        view! {
                                            <>
                                                <div></div>
                                            </>
                                        }
                                            .into_any()
                                    }
                                }
                            })
                    }}
                </div>
            </Suspense>

            <Suspense fallback=|| {
                view! { <p class="text-center text-teal-600 dark:text-aqua-400">"Loading..."</p> }
            }>
                {move || {
                    match poasts.get() {
                        Some(Ok(posts)) => {
                            if posts.is_empty() {
                                view! {
                                    <div class="text-center text-gray-500 dark:text-gray-400">
                                        "No posts found"
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                        <For
                                            each=move || posts.clone()
                                            key=|poast| poast.id
                                            children=move |poast| {
                                                view! {
                                                    <BlogPoast
                                                        poast=poast
                                                        search_term=search_params.get().query
                                                    />
                                                }
                                            }
                                        />
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Some(Err(e)) => {
                            console_log!("Error loading posts: {:?}", e);
                            view! {
                                <div class="text-center text-red-500">"Error loading posts"</div>
                            }
                                .into_any()
                        }
                        None => {
                            view! {
                                <div class="text-center text-gray-500 dark:text-gray-400">
                                    "Loading..."
                                </div>
                            }
                                .into_any()
                        }
                    }
                }}
            </Suspense>
        </div>
    }
}

#[component]
pub fn BlogPoast(
    poast: Poast,
    #[prop(into, optional)] search_term: String,
) -> impl IntoView {
    let company = Memo::new(move |_| poast.company.clone());
    let (is_expanded, set_is_expanded) = signal(false);
    
    let handle_show_more = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        ev.prevent_default();
        set_is_expanded.update(|expanded| *expanded = !*expanded);
    };
    
    // Function to determine background and border color classes based on similarity score
    let get_similarity_colors = move |similarity: Option<i32>| -> (&'static str, &'static str, &'static str) {
        match similarity {
            // High similarity - aquamarine spectrum
            Some(score) if score >= 90 => (
                "bg-aquamarine-light bg-opacity-40 dark:bg-aquamarine-light dark:bg-opacity-20", 
                "border border-aquamarine-dark dark:border-aquamarine",
                "text-aquamarine-dark dark:text-aquamarine"
            ),
            // Medium similarity - purple spectrum
            Some(score) if score >= 75 => (
                "bg-purple-light bg-opacity-40 dark:bg-purple-light dark:bg-opacity-20", 
                "border border-purple-dark dark:border-purple",
                "text-purple-dark dark:text-purple"
            ),
            // Low similarity - orange spectrum
            Some(score) if score > 0 => (
                "bg-orange-light bg-opacity-40 dark:bg-orange-light dark:bg-opacity-20", 
                "border border-orange-dark dark:border-orange",
                "text-orange-dark dark:text-orange"
            ),
            // No similarity data
            _ => (
                "bg-gray-400 bg-opacity-20 dark:bg-gray-600 dark:bg-opacity-20", 
                "border border-gray-600 dark:border-gray-400",
                "text-gray-700 dark:text-gray-400"
            )
        }
    };

    // Format similarity percentage if available
    let similarity_text = move || -> String {
        match poast.similarity {
            Some(score) => format!("{score}%"),
            None => "".to_string()
        }
    };

    view! {
        <div class="relative p-4">
            <article class="base-poast flex flex-col items-start h-full w-full bg-white dark:bg-teal-800 border-2 border-gray-200 dark:border-teal-700 hover:border-seafoam-500 dark:hover:border-aqua-500 p-4 rounded-lg shadow-md hover:shadow-lg transition-all">
                {move || {
                    poast
                        .similarity
                        .map(|_| {
                            view! {
                                <div class="absolute top-5 right-5 flex items-center">
                                    {
                                        let (bg_class, border_class, text_class) = get_similarity_colors(
                                            poast.similarity,
                                        );
                                        view! {
                                            <div class=format!(
                                                "px-2.5 py-1 rounded-full text-xs font-medium {} {} {}",
                                                bg_class,
                                                border_class,
                                                text_class,
                                            )>{similarity_text()}</div>
                                        }
                                    }
                                </div>
                            }
                        })
                }}
                <a
                    href=poast.link.clone()
                    class="block w-full"
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    <div class="flex items-center pb-2 max-w-1/2">
                        {move || {
                            let company_val = company.get();
                            poast
                                .links
                                .clone()
                                .and_then(|links| links.logo_url)
                                .map(|url| {
                                    view! {
                                        <img
                                            src=url
                                            alt=format!("{} logo", company_val)
                                            class="w-8 h-8 mr-2 rounded-sm"
                                        />
                                    }
                                })
                        }}
                        <h2 class="text-sm md:text-base lg:text-lg text-teal-600 dark:text-mint-400 font-semibold truncate">
                            {move || company.get()}
                        </h2>
                    </div>
                    <div class="poast-headings flex flex-col w-full space-y-1">
                        <p class="text-sm md:text-base lg:text-lg text-gray-800 dark:text-gray-200">
                            <HighlightedText
                                text=Cow::from(poast.title.clone())
                                search_term=search_term.clone()
                                class="text-sm md:text-base lg:text-lg text-seafoam-600 dark:text-aqua-400 line-clamp-1 md:line-clamp-2 lg:line-clamp-2 font-medium"
                            />
                        </p>
                        <p class="text-xs md:text-sm lg:text-base text-gray-500 dark:text-gray-400">
                            {poast.published_at.clone()}
                        </p>
                    </div>
                </a> {}
                <div class="poast-summary mt-2 w-full">
                    {move || {
                        poast
                            .summary
                            .clone()
                            .map(|summary| {
                                view! {
                                    <div>
                                        <HighlightedText
                                            text=Cow::from(summary)
                                            search_term=search_term.clone()
                                            class=if is_expanded() {
                                                "text-xs md:text-sm lg:text-base text-gray-600 dark:text-gray-300"
                                            } else {
                                                "text-xs md:text-sm lg:text-base text-gray-600 dark:text-gray-300 line-clamp-2 md:line-clamp-3 lg:line-clamp-4"
                                            }
                                        />
                                        <button
                                            on:click=handle_show_more
                                            class="mt-2 text-xs md:text-sm text-seafoam-600 dark:text-aqua-400 hover:text-seafoam-700 dark:hover:text-aqua-300 transition-colors"
                                        >
                                            {move || {
                                                if is_expanded() { "Show Less" } else { "Show More" }
                                            }}
                                        </button>
                                    </div>
                                }
                            })
                    }}
                </div>
            </article>
        </div>
    }
}

// Helper function to get highlighted segments
fn get_highlighted_segments(text: &str, search_term: &str) -> Vec<(String, bool)> {
    if search_term.is_empty() {
        return vec![(text.to_string(), false)];
    }

    let search_term = search_term.to_lowercase();
    let mut result = Vec::new();
    let mut last_index = 0;
    let text_lower = text.to_lowercase();

    while let Some(start_idx) = text_lower[last_index..].find(&search_term) {
        let absolute_start = last_index + start_idx;
        let absolute_end = absolute_start + search_term.len();

        // Add non-matching segment if there is one
        if absolute_start > last_index {
            result.push((text[last_index..absolute_start].to_string(), false));
        }

        // Add matching segment (using original case from text)
        result.push((text[absolute_start..absolute_end].to_string(), true));

        last_index = absolute_end;
    }

    // Add remaining text if any
    if last_index < text.len() {
        result.push((text[last_index..].to_string(), false));
    }

    result
}

#[component]
fn HighlightedText<'a>(
    #[prop(into)] text: Cow<'a, str>,
    #[prop(into)] search_term: String,
    #[prop(optional)] class: &'static str,
) -> impl IntoView {
    let segments = get_highlighted_segments(&text, &search_term);

    view! {
        <span class=class>
            {segments
                .into_iter()
                .map(|(text, is_highlight)| {
                    if is_highlight {
                        view! {
                            <mark class="bg-mint-400 dark:bg-mint-900 text-seafoam-900 dark:text-seafoam-200 rounded px-0.5">
                                {text}
                            </mark>
                        }
                            .into_any()
                    } else {
                        view! { <span>{text}</span> }.into_any()
                    }
                })
                .collect_view()}
        </span>
    }
}

#[derive(Debug, Serialize)]
pub struct PostEmbedding {
    pub link: String,
    pub embedding: Vec<f32>,
    pub minilm: Vec<f32>,
}

impl<'de> Deserialize<'de> for PostEmbedding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct PostEmbeddingVisitor;

        impl<'de> Visitor<'de> for PostEmbeddingVisitor {
            type Value = PostEmbedding;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct PostEmbedding")
            }

            fn visit_map<V>(self, mut map: V) -> Result<PostEmbedding, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut link = None;
                let mut embedding = None;
                let mut minilm = None;

                fn parse_embedding_value<E: de::Error>(value: serde_json::Value) -> Result<Vec<f32>, E> {
                    if value.is_null() {
                        return Ok(Vec::new());
                    }
                    
                    match value {
                        serde_json::Value::String(s) => {
                            if s.trim().is_empty() || s == "null" {
                                Ok(Vec::new())
                            } else {
                                parse_embedding_string(&s).map_err(de::Error::custom)
                            }
                        },
                        serde_json::Value::Null => Ok(Vec::new()),
                        _ => Err(de::Error::custom("embedding must be a string or null"))
                    }
                }

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "link" => {
                            if link.is_some() {
                                return Err(de::Error::duplicate_field("link"));
                            }
                            link = Some(map.next_value()?);
                        }
                        "embedding" => {
                            if embedding.is_some() {
                                return Err(de::Error::duplicate_field("embedding"));
                            }
                            let value: serde_json::Value = map.next_value()?;
                            embedding = Some(parse_embedding_value(value)?);
                        }
                        "minilm" => {
                            if minilm.is_some() {
                                return Err(de::Error::duplicate_field("minilm"));
                            }
                            let value: serde_json::Value = map.next_value()?;
                            minilm = Some(parse_embedding_value(value)?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                let link = link.ok_or_else(|| de::Error::missing_field("link"))?;
                let embedding = embedding.unwrap_or_default();
                let minilm = minilm.unwrap_or_default();

                Ok(PostEmbedding { link, embedding, minilm })
            }
        }

        const FIELDS: &[&str] = &["link", "embedding", "minilm"];
        deserializer.deserialize_struct("PostEmbedding", FIELDS, PostEmbeddingVisitor)
    }
}

fn parse_embedding_string(s: &str) -> Result<Vec<f32>, String> {
    if s.trim().is_empty() || s == "null" {
        return Ok(Vec::new());
    }
    
    let s = s.trim_start_matches('[')
        .trim_end_matches(']');
    
    s.split(',')
        .map(|num| {
            num.trim().parse::<f32>().map_err(|e| format!("Failed to parse number: {e}"))
        })
        .collect()
}

#[cfg(feature = "ssr")]
async fn get_openai_embedding(query: &str) -> Result<Vec<f32>, ServerFnError> {
    use async_openai::{
        types::{CreateEmbeddingRequestArgs, EmbeddingInput},
        Client,
    };
    let openai = Client::new();
    openai
        .embeddings()
        .create(CreateEmbeddingRequestArgs::default()
            .model("text-embedding-3-small")
            .input(EmbeddingInput::String(query.to_string()))
            .build()
            .map_err(|e| ServerFnError::new(format!("Failed to build embedding request: {e}")))?)
        .await
        .map_err(|e| ServerFnError::new(format!("OpenAI API error: {e}")))
        .map(|response| response.data[0].embedding.clone())
}

#[server(SearchPosts, "/api")]
pub async fn semantic_search(query: String, search_type: SearchType) -> Result<Vec<Poast>, ServerFnError> {
    use crate::embeddings_service::embeddings_local::LocalEmbeddingService;
    use log::{info, warn, error, debug};

    info!("Starting semantic search with query: {query} using {search_type:?}");

    let query_embedding = match search_type {
        SearchType::OpenAISemantic => {
            info!("Using OpenAi embeddings");
            get_openai_embedding(&query).await?
        }
        SearchType::LocalSemantic => {
            info!("Using local embeddings");
            LocalEmbeddingService::init()?;
            match LocalEmbeddingService::get_instance() {
                Ok(service) => service.generate_embedding(&query)
                    .map_err(|e| ServerFnError::new(format!("Local embedding error: {e}")))?,
                Err(e) => {
                    error!("Failed to get local embedding service: {e}");
                    return Err(ServerFnError::new("Local embeddings not available"));
                }
            }
        }
        SearchType::Basic => return Err(ServerFnError::new("Invalid search type"))
    };

    info!("Query embedding generated, length: {}", query_embedding.len());

    // Fetch all embeddings using pagination
    let supabase = crate::supabase::get_client();
    info!("Fetching embeddings from Supabase with pagination");
    
    let mut all_embeddings: Vec<PostEmbedding> = Vec::new();
    let page_size = 1000;
    let mut current_page = 0;
    
    loop {
        let start = current_page * page_size;
        let end = start + page_size - 1;
        
        info!("Fetching embeddings page {}: range {}-{}", current_page + 1, start, end);
        
        let response = supabase
            .from("post_embeddings")
            .select("*")
            .range(start, end)
            .execute()
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to fetch embeddings: {e}")))?;

        let response_text = response.text().await
            .map_err(|e| ServerFnError::new(format!("Failed to get response text: {e}")))?;

        let embeddings_value: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| {
                error!("Failed to parse embeddings page {}: {}", current_page + 1, e);
                error!("First 500 chars of response: {}", &response_text[..response_text.len().min(500)]);
                ServerFnError::new(format!("Failed to parse embeddings: {e}"))
            })?;
            
        if let serde_json::Value::Array(arr) = embeddings_value {
            if arr.is_empty() {
                // No more records, exit the loop
                break;
            }
            
            let page_embeddings: Vec<PostEmbedding> = arr.iter()
                .filter_map(|v| {
                    let result = serde_json::from_value(v.clone());
                    if let Err(ref e) = result {
                        error!("Failed to parse embedding: {e}");
                    }
                    result.ok()
                })
                .collect();
                
            info!("Successfully parsed {} embeddings from page {}", page_embeddings.len(), current_page + 1);
            all_embeddings.extend(page_embeddings);
            current_page += 1;
        } else {
            error!("Expected array response from Supabase, got: {}", &response_text[..response_text.len().min(500)]);
            break;
        }
    }
    
    info!("Successfully fetched and parsed {} embeddings across {} pages", all_embeddings.len(), current_page);

    let mut results: Vec<(String, f32)> = all_embeddings
        .into_iter()
        .filter_map(|post| {
            let embedding_to_compare = match search_type {
                SearchType::OpenAISemantic => {
                    if post.embedding.is_empty() {
                        warn!("Skipping post {} - no OpenAI embedding", post.link);
                        return None;
                    }
                    &post.embedding
                },
                SearchType::LocalSemantic => {
                    if post.minilm.is_empty() {
                        warn!("Skipping post {} - no local embedding", post.link);
                        return None;
                    }
                    &post.minilm
                },
                SearchType::Basic => return None,
            };
    
            let similarity = cosine_similarity(&query_embedding, embedding_to_compare);
            debug!("Similarity for {}: {}", post.link, similarity);
            Some((post.link, similarity))
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    info!("Sorted {} results by similarity", results.len());

    let links: Vec<String> = results.iter()
        .take(30)
        .map(|(link, score)| {
            debug!("Selected result - link: {link}, score: {score}");
            link.clone()
        })
        .collect();

    info!("Fetching full post data for top {} results", links.len());
    let posts_response = supabase
        .from("poasts")
        .select("id, published_at, company, title, link, summary, links!posts_company_fkey(logo_url)")
        .in_("link", &links)
        .execute()
        .await?;

    let posts_text = posts_response.text().await?;
    debug!("Posts response: {posts_text}");

    let parse_posts_result = serde_json::from_str::<Vec<Poast>>(&posts_text);
    
    match &parse_posts_result {
        Ok(posts) => info!("Successfully parsed {} posts", posts.len()),
        Err(e) => {
            error!("Failed to parse posts: {e}");
            error!("First 500 chars of posts response: {}", &posts_text[..posts_text.len().min(500)]);
            return Err(ServerFnError::ServerError(format!("Failed to parse posts: {e}")));
        }
    }

    let mut posts = parse_posts_result?;

    info!("Adding similarity scores to posts");
    for post in &mut posts {
        if let Some((_, score)) = results.iter().find(|(l, _)| l == &post.link) {
            // Convert similarity score to percentage (0-100)
            let percentage = (score * 100.0).round() as i32;
            post.similarity = Some(percentage);
            debug!("Post '{}' has similarity score: {}%", post.title, percentage);
        }
    }

    info!("Sorting posts by similarity scores");
    posts.sort_by(|a, b| {
        let a_score = results.iter().find(|(l, _)| l == &a.link).unwrap().1;
        let b_score = results.iter().find(|(l, _)| l == &b.link).unwrap().1;
        b_score.partial_cmp(&a_score).unwrap()
    });

    info!("Returning {} ranked posts", posts.len());
    Ok(posts)
}

#[cfg(feature = "ssr")]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}


