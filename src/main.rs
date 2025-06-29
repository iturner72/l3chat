use cfg_if::cfg_if;
use tracing_subscriber::fmt::format::FmtSpan;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::{
            body::Body as AxumBody,
            extract::State,
            http::Request,
            response::IntoResponse,
            routing::get,
            middleware,
            Router,
        };
        use dashmap::DashMap;
        use dotenv::dotenv;
        use tracing_subscriber::EnvFilter;
        use l3chat::state::AppState;
        use leptos::prelude::*;
        use leptos_axum::{generate_route_list, handle_server_fns_with_context, LeptosRoutes};
        use l3chat::app::*;
        use l3chat::auth::server::middleware::require_auth_no_db;
        use l3chat::auth::oauth::{google_login, discord_login, google_callback, discord_callback};
        use l3chat::cancellable_sse::*;
        use l3chat::database::db::establish_connection;
        use l3chat::handlers::sse::{
            create_stream,
            send_message_stream_handler,
            title_updates_handler,
        };
        use l3chat::middleware::tracing::{ColoredFields, trace_requests};
        use std::net::SocketAddr;
        use std::sync::Arc;

        #[tokio::main]
        async fn main() {
            dotenv().ok();

            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("info,l3chat=debug"))
                )
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_span_events(FmtSpan::CLOSE)
                .pretty()
                .fmt_fields(ColoredFields)
                .init();

            let conf = get_configuration(None).unwrap();
            let addr = conf.leptos_options.site_addr;
            let leptos_options = conf.leptos_options;

            let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
            let pool = establish_connection(&database_url).expect("Failed to create database pool");

            let routes = generate_route_list(App);

            let app_state = AppState {
                leptos_options: leptos_options.clone(),
                pool,
                sse_state: SseState::new(),
                oauth_states: Arc::new(dashmap::DashMap::new()),
                title_update_senders: Arc::new(DashMap::new())
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

            // OAuth routes (public)
            let oauth_routes = Router::new()
                .route("/auth/google", get(google_login))
                .route("/auth/google-callback", get(google_callback))
                .route("/auth/discord", get(discord_login))
                .route("/auth/discord/callback", get(discord_callback));

            let protected_routes = Router::new()
                .route("/api/create-stream", get(create_stream))
                .route("/api/cancel-stream", get(cancel_stream))
                .route("/api/send_message_stream", get(send_message_stream_handler))
                .route("/api/title-updates", get(title_updates_handler))
                .layer(middleware::from_fn_with_state(
                    app_state.clone(),
                    require_auth_no_db
                ));

            let app = Router::new()
                .route(
                    "/api/{*fn_name}",
                    get(server_fn_handler).post(server_fn_handler),
                )
                .merge(oauth_routes)
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
                .layer(middleware::from_fn(trace_requests))
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
