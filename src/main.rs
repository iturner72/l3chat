use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::{
            body::Body as AxumBody,
            extract::{Query, State},
            http::Request,
            response::IntoResponse,
            routing::get,
            middleware,
            Router,
            response::sse::Sse,
        };
        use dotenv::dotenv;
        use env_logger::Env;
        use l3chat::state::AppState;
        use leptos::prelude::*;
        use leptos_axum::{generate_route_list, handle_server_fns_with_context, LeptosRoutes};
        use l3chat::app::*;
        use l3chat::auth::server::middleware::require_auth;
        use l3chat::cancellable_sse::*;
        use l3chat::components::chat::{SseStream, send_message_stream};
        use l3chat::database::db::establish_connection;
        use l3chat::handlers::*;
        use std::collections::HashMap;
        use std::net::SocketAddr;
        use std::sync::{Arc,Mutex};
        use tokio::sync::{mpsc, broadcast};

        #[tokio::main]
        async fn main() {
            dotenv().ok();
            env_logger::init_from_env(Env::default().default_filter_or("info"));


            let conf = get_configuration(None).unwrap();
            let addr = conf.leptos_options.site_addr;
            let leptos_options = conf.leptos_options;

            let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
            let pool = establish_connection(&database_url).expect("Failed to create database pool");

            // Generate the list of routes in your Leptos App
            let routes = generate_route_list(App);

            let app_state = AppState {
                leptos_options: leptos_options.clone(),
                pool,
                sse_state: SseState::new(),
                drawing_tx: broadcast::Sender::new(100),
                user_count: Arc::new(Mutex::new(0)),
            };

            async fn server_fn_handler(
                State(app_state): State<AppState>,
                request: Request<AxumBody>,
            ) -> impl IntoResponse {
                handle_server_fns_with_context(
                    move || {
                        provide_context(app_state.clone());
                    },
                    request,
                )
                .await
            }

            let protected_routes = Router::new()
                .route("/api/create-stream", get(create_stream))
                .route("/api/cancel-stream", get(cancel_stream))
                .route("/api/generate-embeddings", get(embeddings_generation_handler))
                .route("/api/generate-local-embeddings", get(local_embeddings_generation_handler))
                .route("/api/send_message_stream", axum::routing::get(|
                    State(app_state): State<AppState>,
                    Query(params): Query<HashMap<String, String>>
                | async move {
                    let (tx, rx) = mpsc::channel(1);
                    if let (Some(thread_id), Some(model), Some(lab)) = (
                        params.get("thread_id"),
                        params.get("model"),
                        params.get("lab")
                    ) {
                        let pool = app_state.pool.clone();
                        let thread_id = thread_id.clone();
                        let model = model.clone();
                        let lab = lab.clone();
                        tokio::spawn(async move {
                            send_message_stream(&pool, thread_id, model, lab, tx).await;
                        });
                    }
                    Sse::new(SseStream { receiver: rx })
                }))
                .layer(middleware::from_fn(require_auth));

            let app = Router::new()
                .route(
                    "/api/{*fn_name}",
                    get(server_fn_handler).post(server_fn_handler),
                )
                .route("/ws/drawing", get(drawing_ws_handler))
                .merge(protected_routes)
                .leptos_routes_with_handler(routes, get(|State(app_state): State<AppState>, request: Request<AxumBody>| async move {
                    let handler = leptos_axum::render_app_to_stream_with_context(
                        move || {
                            provide_context(app_state.clone());
                        },
                        move || shell(leptos_options.clone())
                    );
                    handler(request).await.into_response()
                }))
                .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
                .with_state(app_state);

            log::info!("Starting server at {addr}");

            // run our app with hyper
            // `axum::Server` is a re-export of `hyper::Server`
            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            log::info!("listening on http://{}", &addr);
            axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
        }
    } else {
        pub fn main() {
            // no client-side main function
            // unless we want this to work with e.g., Trunk for a purely client-side app
            // see lib.rs for hydration function instead
        }
    }
}
