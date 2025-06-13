use crate::components::auth_nav::AuthNav;
use crate::components::canvas::DrawingCanvas;
use crate::components::footer::Footer;
use leptos::prelude::*;

#[component]
pub fn DrawingPage() -> impl IntoView {
    view! {
        <div class="w-full mx-auto pl-2 bg-gray-100 dark:bg-teal-900">
            <div class="flex justify-between items-center">
                <a

                    href="/"
                    class="text-3xl text-left text-seafoam-600 dark:text-mint-400 ib pl-4 p-4 font-bold"
                >
                    "l3chat"
                </a>
                <AuthNav/>
            </div>

            <div class="container mx-auto py-6">
                <DrawingCanvas/>
            </div>

            <Footer/>
        </div>
    }
}
