use leptos::prelude::*;

#[component]
pub fn Toast(
    message: ReadSignal<String>,
    visible: ReadSignal<bool>,
    #[prop(into)] on_close: Callback<()>,
) -> impl IntoView {
    let opacity_class = move || {
        if visible.get() {
            "opacity-100"
        } else {
            "opacity-0"
        }
    };

    view! {
        <div class=move || {
            format!(
                "{} fixed bottom-4 right-4 bg-gray-100 dark:bg-rich-black-500 text-teal-500 dark:text-mint-300 px-4 py-2 rounded shadow-lg transition-opacity duration-300",
                opacity_class(),
            )
        }>
            {message}
            <button
                class="ml-2 text-salmon-600 hover:text-salmon-700 dark:text-salmon-700 dark:hover:text-salmon-800"
                on:click=move |_| on_close.run(())
            >
                "×"
            </button>
        </div>
    }
}
