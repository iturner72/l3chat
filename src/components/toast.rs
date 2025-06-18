use crate::components::ui::{ButtonVariant, IconButton};
use leptos::prelude::*;
use leptos_icons::Icon;

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
                "{} fixed bottom-4 right-4 bg-gray-100 dark:bg-teal-800 text-mint-800 dark:text-mint-600 px-4 py-2 rounded shadow-lg transition-opacity duration-0",
                opacity_class(),
            )
        }>
            <div class="flex items-center justify-between gap-2">
                <span class="text-mint-800 dark:text-mint-600-">{message}</span>
                <IconButton
                    variant=ButtonVariant::Ghost
                    size=crate::components::ui::ButtonSize::Small
                    on_click=Callback::new(move |_| on_close.run(()))
                    class="ml-2 text-danger-500 hover:text-danger-600"
                >
                    {move || {
                        if visible.get() {
                            view! { <Icon icon=icondata_bs::BsXCircle width="16" height="16"/> }
                        } else {
                            view! { <Icon icon=icondata_bs::BsXCircle width="16" height="16"/> }
                        }
                    }}

                </IconButton>
            </div>
        </div>
    }
}
