use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <div class="flex flex-row left-0 pl-4 p-4 space-x-2 text-teal-400 dark-text-teal-600">
            <span>"summaries by gpt-4o-mini"</span>
            <span>"•"</span>
            <span>"inspired by"</span>
            <a href="https://www.ishanshah.me/" class="font-bold">
                "ishan0102's"
            </a>
            <span>"•"</span>
            <a href="https://github.com/ishan0102/engblogs" class="font-extrabold">
                "engblogs"
            </a>
        </div>
    }
}
