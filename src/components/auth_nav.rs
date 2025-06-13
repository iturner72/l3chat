use leptos::prelude::*;

use crate::components::dark_mode_toggle::DarkModeToggle;

#[component]
pub fn AuthNav() -> impl IntoView {
    view! {
        <div class="items-end pr-4 flex space-x-4">
            <DarkModeToggle/>
        </div>
    }
}
