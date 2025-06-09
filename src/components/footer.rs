use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <div class="flex flex-row left-0 pl-4 p-4 space-x-2 text-teal-400 dark-text-teal-600">
            <span>"t3"</span>
            <span>"•"</span>
            <span>"chat"</span>
            <span>"•"</span>
            <a
                href="https://www.cloneathon.t3.chat"
                class="font-bold"
                target="_blank"
                rel="noopener noreferrer"
            >
                "cloneathon"
            </a>
        </div>
    }
}
