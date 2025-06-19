use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Outline,
    Ghost,
    Danger,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonSize {
    Tiny,
    Small,
    Medium,
    Large,
}

impl ButtonVariant {
    fn get_classes(&self) -> &'static str {
        match self {
            ButtonVariant::Primary => {
                "bg-gray-700 dark:bg-teal-600 text-white \
                 hover:bg-gray-700 dark:hover:bg-teal-600 \
                 active:bg-teal-800 dark:active:bg-teal-500 \
                 border border-gray-300 dark:border-teal-700"
            }
            ButtonVariant::Secondary => {
                "bg-gray-400 dark:bg-gray-600 text-gray-900 dark:text-gray-100 \
                 hover:bg-gray-500 dark:hover:bg-gray-500 \
                 focus:ring-2 focus:ring-gray-400 dark:focus:ring-gray-500 \
                 active:bg-gray-600 dark:active:bg-gray-400 \
                 border border-gray-400 dark:border-gray-600"
            }
            ButtonVariant::Outline => {
                "bg-transparent border-2 \
                 border-gray-600 dark:border-gray-900 \
                 text-gray-700 dark:text-gray-300 \
                 hover:bg-gray-100 dark:hover:bg-gray-800 \
                 hover:border-gray-800 dark:hover:border-gray-200 \
                 focus:ring-2 focus:ring-gray-500 dark:focus:ring-gray-700 \
                 active:bg-gray-200 dark:active:bg-gray-700"
            }
            ButtonVariant::Ghost => {
                "bg-transparent text-gray-600 dark:text-gray-400 \
                 hover:bg-gray-100 dark:hover:bg-gray-800 \
                 hover:text-gray-800 dark:hover:text-gray-200 \
                 focus:ring-2 focus:ring-gray-400 dark:focus:ring-gray-500 \
                 active:bg-gray-200 dark:active:bg-gray-700"
            }
            ButtonVariant::Danger => {
                "bg-salmon-500 dark:bg-salmon-600 text-white \
                 hover:bg-salmon-600 dark:hover:bg-salmon-700 \
                 focus:ring-2 focus:ring-salmon-400 dark:focus:ring-salmon-500 \
                 active:bg-salmon-700 dark:active:bg-salmon-800 \
                 border border-salmon-500 dark:border-salmon-600"
            }
            ButtonVariant::Success => {
                "bg-seafoam-400 dark:bg-seafoam-500 text-gray-600 dark-text-gray-400 \
                 hover:bg-seafoam-600 dark:hover:bg-seafoam-700 \
                 focus:ring-2 focus:ring-seafoam-500 dark:focus:ring-seafoam-600 \
                 active:bg-seafoam-700 dark:active:bg-seafoam-800 \
                 border border-seafoam-500 dark:border-seafoam-600"
            }
        }
    }
}

impl ButtonSize {
    fn get_classes(&self) -> &'static str {
        match self {
            ButtonSize::Tiny => "text-xs",
            ButtonSize::Small => "px-2 py-1 text-xs",
            ButtonSize::Medium => "px-3 py-2 text-sm",
            ButtonSize::Large => "px-2 py-1 text-base",
        }
    }
}

#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] full_width: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional)] on_click: Option<Callback<web_sys::MouseEvent>>,
    children: Children,
) -> impl IntoView {
    let variant = if disabled {
        ButtonVariant::Secondary
    } else {
        variant
    };
    let base_classes = "inline-flex items-center justify-center font-medium rounded transition-all duration-0 focus:outline-none";
    let variant_classes = variant.get_classes();
    let size_classes = size.get_classes();

    let disabled_classes = if disabled {
        "opacity-50 cursor-not-allowed pointer-events-none"
    } else {
        "cursor-pointer"
    };

    let width_classes = if full_width { "w-full" } else { "" };

    let combined_classes = format!(
        "{} {} {} {} {} {}",
        base_classes, variant_classes, size_classes, disabled_classes, width_classes, class
    );

    view! {
        <button
            class=combined_classes
            disabled=disabled
            on:click=move |ev| {
                if !disabled {
                    if let Some(handler) = on_click {
                        handler.run(ev);
                    }
                }
            }
        >

            {children()}
        </button>
    }
}

#[component]
pub fn IconButton(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional)] on_click: Option<Callback<web_sys::MouseEvent>>,
    children: Children,
) -> impl IntoView {
    let size_padding = match size {
        ButtonSize::Tiny => "p-0",
        ButtonSize::Small => "p-1",
        ButtonSize::Medium => "p-2",
        ButtonSize::Large => "p-3",
    };

    view! {
        <Button
            variant=variant
            disabled=disabled
            class=format!("{} {} aspect-square", size_padding, class)
            on_click=on_click.expect("something")
        >
            {children()}
        </Button>
    }
}

#[component]
pub fn LinkButton(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] full_width: bool,
    #[prop(optional, into)] class: String,
    #[prop(into)] href: String,
    #[prop(optional)] target: String,
    children: Children,
) -> impl IntoView {
    let variant = if disabled {
        ButtonVariant::Secondary
    } else {
        variant
    };
    let base_classes = "inline-flex items-center justify-center font-medium rounded transition-all duration-150 focus:outline-none no-underline";
    let variant_classes = variant.get_classes();
    let size_classes = size.get_classes();

    let disabled_classes = if disabled {
        "opacity-50 cursor-not-allowed pointer-events-none"
    } else {
        "cursor-pointer"
    };

    let width_classes = if full_width { "w-full" } else { "" };

    let combined_classes = format!(
        "{} {} {} {} {} {}",
        base_classes, variant_classes, size_classes, disabled_classes, width_classes, class
    );

    view! {
        <a class=combined_classes href=href target=target>
            {children()}
        </a>
    }
}

impl Default for ButtonVariant {
    fn default() -> Self {
        ButtonVariant::Primary
    }
}

impl Default for ButtonSize {
    fn default() -> Self {
        ButtonSize::Medium
    }
}
