use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    path, StaticSegment,
};

use crate::auth::auth_components::{AdminLogin, ProtectedAdminPanel};
use crate::components::auth_nav::AuthNav;
use crate::components::drawing::DrawingPage;
use crate::components::footer::Footer;
use crate::components::poasts::Poasts;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/l3chat.css" />
        <Title text="Welcome to Leptos" />
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=path!("admin") view=AdminLogin />
                    <Route path=path!("admin-panel") view=ProtectedAdminPanel />
                    <Route path=path!("draw") view=DrawingPage />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="w-full mx-auto pl-2 bg-gray-100 dark:bg-teal-900">
            <div class="flex justify-between items-center">
                <a
                    href="/"
                    class="text-3xl text-left text-seafoam-600 dark:text-mint-400 ib pl-4 p-4 font-bold"
                >
                    "l3chat"
                </a>
                <AuthNav />
            </div>
            <Poasts />
            <div class="container mx-auto p-4 flex justify-center">
                <a
                    href="/draw"
                    class="bg-teal-500 hover:bg-teal-600 text-white font-bold py-2 px-4 rounded transition-colors"
                >
                    "Try Collaborative Drawing"
                </a>
            </div>
            <Footer />
        </div>
    }
}
