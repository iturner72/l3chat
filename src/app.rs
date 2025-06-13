use leptos::prelude::*;
use leptos_fetch::QueryClient;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    path, StaticSegment,
};
use std::time::Duration;

use crate::auth::auth_components::{AdminLogin, ProtectedAdminPanel};
use crate::auth::context::AuthProvider;
use crate::components::drawing::DrawingPage;
use crate::pages::writersroom::WritersRoom;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    // Initialize QueryClient with global options and provide to context
    let client = QueryClient::new().with_options(
        leptos_fetch::QueryOptions::new()
            .with_stale_time(Duration::from_secs(60))
            .with_gc_time(Duration::from_secs(300)),
    );

    client.provide();

    view! {
        <Stylesheet id="leptos" href="/pkg/l3chat.css"/>
        <Title text="l3chat"/>
        <AuthProvider>
            <Router>
                <main>
                    <Routes fallback=|| "Page not found.".into_view()>
                        <Route path=StaticSegment("") view=WritersRoom/>
                        <Route path=path!("admin") view=AdminLogin/>
                        <Route path=path!("admin-panel") view=ProtectedAdminPanel/>
                        <Route path=path!("draw") view=DrawingPage/>
                    </Routes>
                </main>
            </Router>
        </AuthProvider>
    }
}
