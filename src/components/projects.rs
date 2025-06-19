use leptos::prelude::*;
use leptos_fetch::QueryClient;
use leptos_icons::Icon;
use uuid::Uuid;
use web_sys::Event;
use wasm_bindgen::JsCast;

use crate::models::projects::*;
use crate::server_fn::projects::*;
use crate::components::ui::{Button, IconButton, ButtonVariant, ButtonSize};
use crate::components::threadlist::get_threads_query;
use crate::pages::writersroom::ThreadContext;

pub async fn get_user_projects_query() -> Result<Vec<ProjectView>, String> {
    get_user_projects().await.map_err(|e| e.to_string())
}

#[component]
pub fn ProjectsPage(
    // Accept project selection state from parent
    selected_project: ReadSignal<Option<Uuid>>,
    set_selected_project: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    let client: QueryClient = expect_context();
    let (show_create_form, set_show_create_form) = signal(false);

    let projects_resource = client.resource(
        get_user_projects_query,
        || () 
    );

    view! {
        <div class="min-h-screen bg-gray-100 dark:bg-teal-900 p-6">
            <div class="max-w-3xl mx-auto">
                <div class="flex justify-between items-center mb-6">
                    <h1 class="text-3xl font-bold text-gray-800 dark:text-gray-200">"Projects"</h1>
                    <Button
                        variant=ButtonVariant::Success
                        on_click=Callback::new(move |_| set_show_create_form.set(true))
                    >
                        "New Project"
                    </Button>
                </div>

                <div class="projects-list-container space-y-6">
                    <div class="projects-list-section">
                        <ProjectsList
                            projects_resource=projects_resource
                            selected_project=selected_project
                            set_selected_project=set_selected_project
                        />
                    </div>

                    <div class="project-details">
                        {move || {
                            if let Some(project_id) = selected_project.get() {
                                view! { <ProjectDetails project_id=project_id/> }.into_any()
                            } else {
                                view! {
                                    <div class="bg-white dark:bg-teal-800 rounded-lg shadow-md p-8 text-center">
                                        <div class="text-gray-500 dark:text-gray-400">
                                            <div class="text-4xl mb-4">"📁"</div>
                                            <h3 class="text-lg font-medium mb-2">"Select a Project"</h3>
                                            <p class="text-sm">
                                                "Choose a project from the list to view its details and documents."
                                            </p>
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                        }}

                    </div>
                </div>

                {move || {
                    if show_create_form.get() {
                        view! {
                            <CreateProjectModal
                                _show=show_create_form
                                set_show=set_show_create_form
                            />
                        }
                            .into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}

            </div>
        </div>
    }.into_any()
}

#[component]
pub fn ProjectsList(
    projects_resource: Resource<Result<Vec<ProjectView>, String>>,
    selected_project: ReadSignal<Option<Uuid>>,
    set_selected_project: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    view! {
        <div class="card-themed p-6">
            <h2 class="text-xl font-semibold text-themed-primary mb-4">"Your Projects"</h2>

            <Transition fallback=|| {
                view! {
                    <div class="space-y-3">
                        <div class="animate-pulse surface-secondary h-16 rounded-md"></div>
                        <div class="animate-pulse surface-secondary h-16 rounded-md"></div>
                        <div class="animate-pulse surface-secondary h-16 rounded-md"></div>
                    </div>
                }
                    .into_any()
            }>
                {move || {
                    match projects_resource.get() {
                        Some(Ok(projects)) => {
                            if projects.is_empty() {
                                view! {
                                    <div class="text-center text-themed-secondary py-8">
                                        <div class="text-2xl mb-2">"📋"</div>
                                        <p class="text-sm">
                                            "No projects yet. Create your first project to get started!"
                                        </p>
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="space-y-3">
                                        <For
                                            each=move || projects.clone()
                                            key=|project| project.id
                                            children=move |project| {
                                                let project_id = project.id;
                                                let project_name = project.name.clone();
                                                let project_description = project.description.clone();
                                                let is_selected = move || {
                                                    selected_project.get().map_or(false, |id| id == project_id)
                                                };
                                                view! {
                                                    <div class="flex flex-row items-center justify-between space-x-2">
                                                        <Button
                                                            variant=if is_selected() {
                                                                ButtonVariant::Success
                                                            } else {
                                                                ButtonVariant::Outline
                                                            }

                                                            full_width=true
                                                            class=format!(
                                                                "text-left p-4 group transition-all duration-200 {}",
                                                                if is_selected() {
                                                                    "ring-2 ring-mint-400 dark:ring-mint-500 bg-mint-50 dark:bg-mint-900"
                                                                } else {
                                                                    ""
                                                                },
                                                            )

                                                            on_click=Callback::new(move |_| {
                                                                if selected_project.get() == Some(project_id) {
                                                                    set_selected_project.set(None);
                                                                } else {
                                                                    set_selected_project.set(Some(project_id));
                                                                }
                                                            })
                                                        >

                                                            <div class="flex flex-row items-center justify-between gap-2 w-full">
                                                                <div class="flex items-center gap-3">
                                                                    <div class="flex-shrink-0">
                                                                        {move || {
                                                                            if is_selected() {
                                                                                view! {
                                                                                    <Icon
                                                                                        icon=icondata_bs::BsFolder2Open
                                                                                        width="20"
                                                                                        height="20"
                                                                                    />
                                                                                }
                                                                                    .into_any()
                                                                            } else {
                                                                                view! {
                                                                                    <Icon icon=icondata_bs::BsFolder2 width="20" height="20"/>
                                                                                }
                                                                                    .into_any()
                                                                            }
                                                                        }}

                                                                    </div>
                                                                    <div class="flex-1 min-w-0">
                                                                        <h3 class="font-medium">{project_name.clone()}</h3>
                                                                        {project_description
                                                                            .as_ref()
                                                                            .map(|desc| {
                                                                                view! {
                                                                                    <p class="text-sm opacity-75 mt-1 truncate">
                                                                                        {desc.clone()}
                                                                                    </p>
                                                                                }
                                                                                    .into_any()
                                                                            })}

                                                                    </div>
                                                                </div>
                                                                {move || {
                                                                    if is_selected() {
                                                                        view! {
                                                                            <div class="flex-shrink-0 text-success-600 dark:text-success-400">
                                                                                <Icon
                                                                                    icon=icondata_bs::BsCheck2Circle
                                                                                    width="16"
                                                                                    height="16"
                                                                                />
                                                                            </div>
                                                                        }
                                                                            .into_any()
                                                                    } else {
                                                                        view! { <div></div> }.into_any()
                                                                    }
                                                                }}

                                                            </div>
                                                        </Button>
                                                        <DeleteProjectButton
                                                            project_id=project_id
                                                            project_name=project.name.clone()
                                                            set_selected_project=set_selected_project
                                                        />
                                                    </div>
                                                }
                                                    .into_any()
                                            }
                                        />

                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Some(Err(e)) => {
                            view! {
                                // Toggle project selection

                                <div class="text-center error-themed py-8">
                                    <p>"Error loading projects: " {e}</p>
                                </div>
                            }
                                .into_any()
                        }
                        None => {
                            view! {
                                // Toggle project selection

                                <div></div>
                            }
                                .into_any()
                        }
                    }
                }}

            </Transition>
        </div>
    }.into_any()
}

#[component]
pub fn DeleteProjectButton(
    project_id: Uuid,
    project_name: String,
    set_selected_project: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    let client: QueryClient = expect_context();
    let (show_confirm, set_show_confirm) = signal(false);

    let delete_action = Action::new(move |&project_id: &Uuid| {
        async move {
            match delete_project(project_id).await {
                Ok(_) => {
                    // Clear selection if this project was selected
                    set_selected_project.update(|selected| {
                        if *selected == Some(project_id) {
                            *selected = None;
                        }
                    });
                    
                    client.invalidate_query(get_user_projects_query, ());
                    client.invalidate_query(get_threads_query, ());
                    Ok(())
                }
                Err(e) => Err(format!("Failed to delete project: {}", e))
            }
        }
    });

    view! {
        <div class="relative">
            <IconButton
                variant=ButtonVariant::Ghost
                size=ButtonSize::Small
                class="opacity-80 group-hover:opacity-100 transition-opacity text-danger-500 hover:text-danger-600"
                on_click=Callback::new(move |ev: web_sys::MouseEvent| {
                    ev.stop_propagation();
                    set_show_confirm.set(true);
                })
            >

                <Icon icon=icondata_bs::BsTrash3 width="16" height="16"/>
            </IconButton>

            {move || {
                if show_confirm.get() {
                    view! {
                        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
                            <div class="card-themed p-6 w-full max-w-md">
                                <div class="flex items-center mb-4">
                                    <div class="text-danger-500 text-2xl mr-3">"⚠️"</div>
                                    <h3 class="text-lg font-semibold text-themed-primary">
                                        "Delete Project"
                                    </h3>
                                </div>

                                <div class="mb-6">
                                    <p class="text-themed-primary mb-2">
                                        "Are you sure you want to delete the project:"
                                    </p>
                                    <p class="font-medium text-themed-primary surface-secondary p-2 rounded">
                                        "\"" {project_name.clone()} "\""
                                    </p>
                                    <p class="text-sm text-salmon-500 mt-3">
                                        "This will permanently delete:"
                                    </p>
                                    <ul class="text-sm text-salmon-500 mt-1 ml-4">
                                        <li>"• All project documents and embeddings"</li>
                                        <li>"• All chat threads associated with this project"</li>
                                        <li>"• All messages in those threads"</li>
                                    </ul>
                                    <p class="ib text-sm font-medium text-salmon-600 mt-2">
                                        "This action cannot be undone."
                                    </p>
                                </div>

                                <div class="flex justify-end space-x-3">
                                    <Button
                                        variant=ButtonVariant::Ghost
                                        disabled=delete_action.pending().get()
                                        on_click=Callback::new(move |_| set_show_confirm.set(false))
                                    >
                                        "Cancel"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Danger
                                        disabled=delete_action.pending().get()
                                        on_click=Callback::new(move |_| {
                                            delete_action.dispatch(project_id);
                                            set_show_confirm.set(false);
                                        })
                                    >

                                        {move || {
                                            if delete_action.pending().get() {
                                                "Deleting..."
                                            } else {
                                                "Delete Project"
                                            }
                                        }}

                                    </Button>
                                </div>
                            </div>
                        </div>
                    }
                        .into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

        </div>
    }.into_any()
}

#[component]
fn ProjectDetails(project_id: Uuid) -> impl IntoView {
    let _client: QueryClient = expect_context();
    let (show_upload, set_show_upload) = signal(false);

    let documents_resource = Resource::new(
        move || project_id,
        |project_id| async move {
            get_project_documents(project_id).await.map_err(|e| e.to_string())
        }
    );

    view! {
        <div class="card-themed p-6">
            <div class="flex justify-between items-center mb-4">
                <h2 class="text-xl font-semibold text-themed-primary">"Project Details"</h2>
                <div class="flex space-x-2">
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Small
                        on_click=Callback::new(move |_| set_show_upload.set(true))
                    >
                        "Upload Document"
                    </Button>
                    <StartChatButton project_id=project_id/>
                </div>
            </div>

            <Suspense fallback=|| {
                view! { <div class="loading-themed">"Loading documents..."</div> }.into_any()
            }>
                {move || {
                    match documents_resource.get() {
                        Some(Ok(documents)) => {
                            if documents.is_empty() {
                                view! {
                                    <div class="text-center text-themed-secondary py-8">
                                        <div class="text-2xl mb-2">"📄"</div>
                                        <p class="text-sm">"No documents uploaded yet."</p>
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="space-y-2">
                                        <For
                                            each=move || documents.clone()
                                            key=|doc| doc.id
                                            children=|doc| {
                                                view! {
                                                    <div class="surface-secondary p-3 rounded border-themed">
                                                        <div class="flex justify-between items-center">
                                                            <span class="text-themed-primary font-medium">
                                                                {doc.filename}
                                                            </span>
                                                            <span class="text-xs text-themed-secondary">
                                                                {format!("{} chars", doc.content.len())}
                                                            </span>
                                                        </div>
                                                    </div>
                                                }
                                                    .into_any()
                                            }
                                        />

                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Some(Err(e)) => {
                            view! {
                                <div class="error-themed text-center py-4">
                                    "Error loading documents: " {e}
                                </div>
                            }
                                .into_any()
                        }
                        None => view! { <div></div> }.into_any(),
                    }
                }}

            </Suspense>

            {move || {
                if show_upload.get() {
                    view! {
                        <UploadDocumentModal
                            project_id=project_id
                            _show=show_upload
                            set_show=set_show_upload
                            on_uploaded=Callback::new(move |_| {
                                documents_resource.refetch();
                            })
                        />
                    }
                        .into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

        </div>
    }.into_any()
}

#[component]
pub fn StartChatButton(project_id: Uuid) -> impl IntoView {
    let client: QueryClient = expect_context();

    let create_thread_action = Action::new(move |&project_id: &Uuid| {
        async move {
            match create_project_thread(project_id).await {
                Ok(thread_id) => {
                    client.invalidate_query(get_threads_query, ());

                    if let Some(thread_context) = use_context::<ThreadContext>() {
                        thread_context.set_thread_id.set(thread_id.clone());
                        thread_context.set_message_refetch_trigger.update(|n| *n += 1);
                        thread_context.set_pending_messages.update(|msgs| msgs.clear());
                        thread_context.set_search_term.set(String::new());
                    }

                    Ok(thread_id)
                }
                Err(e) => Err(format!("Failed to create thread: {}", e))
            }
        }
    });

    view! {
        <Button
            variant=ButtonVariant::Success
            disabled=create_thread_action.pending().get()
            on_click=Callback::new(move |_| {
                create_thread_action.dispatch(project_id);
            })
        >

            {move || {
                if create_thread_action.pending().get() { "Creating Chat..." } else { "Start Chat" }
            }}

        </Button>
    }.into_any()
}

#[component]
fn CreateProjectModal(
    _show: ReadSignal<bool>,
    set_show: WriteSignal<bool>,
) -> impl IntoView {
    let client: QueryClient = expect_context();
    let (name, set_name) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (instructions, set_instructions) = signal(String::new());

    let create_action = Action::new(move |_: &()| {
        let project_data = NewProjectView {
            name: name.get(),
            description: if description.get().is_empty() { None } else { Some(description.get()) },
            instructions: if instructions.get().is_empty() { None } else { Some(instructions.get()) },
        };

        async move {
            match create_project(project_data).await {
                Ok(_) => {
                    client.invalidate_query(get_user_projects_query, ());
                    set_show.set(false);
                    set_name.set(String::new());
                    set_description.set(String::new());
                    set_instructions.set(String::new());
                    Ok(())
                }
                Err(e) => Err(format!("Failed to create project: {}", e))
            }
        }
    });

    view! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
            <div class="card-themed p-6 w-full max-w-md">
                <div class="flex justify-between items-center mb-4">
                    <h3 class="text-lg font-semibold text-themed-primary">"Create New Project"</h3>
                    <IconButton
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Small
                        class="text-themed-secondary hover:text-themed-primary"
                        on_click=Callback::new(move |_| set_show.set(false))
                    >
                        "✕"
                    </IconButton>
                </div>

                <div class="space-y-4">
                    <div>
                        <label class="block text-sm font-medium text-themed-primary mb-1">
                            "Project Name"
                        </label>
                        <input
                            type="text"
                            class="input-themed w-full"
                            placeholder="Enter project name"
                            prop:value=name
                            on:input=move |ev| set_name.set(event_target_value(&ev))
                        />
                    </div>

                    <div>
                        <label class="block text-sm font-medium text-themed-primary mb-1">
                            "Description (Optional)"
                        </label>
                        <textarea
                            class="input-themed w-full resize-none"
                            rows="3"
                            placeholder="Brief description of your project"
                            prop:value=description
                            on:input=move |ev| set_description.set(event_target_value(&ev))
                        ></textarea>
                    </div>

                    <div>
                        <label class="block text-sm font-medium text-themed-primary mb-1">
                            "Instructions (Optional)"
                        </label>
                        <textarea
                            class="input-themed w-full resize-none"
                            rows="4"
                            placeholder="Special instructions for AI when working with this project"
                            prop:value=instructions
                            on:input=move |ev| set_instructions.set(event_target_value(&ev))
                        ></textarea>
                    </div>

                    <div class="flex justify-end space-x-3 pt-4">
                        <Button
                            variant=ButtonVariant::Ghost
                            on_click=Callback::new(move |_| set_show.set(false))
                        >
                            "Cancel"
                        </Button>
                        <button
                            class="px-4 py-2 bg-seafoam-600 dark:bg-seafoam-700 text-white rounded-md 
                            hover:bg-seafoam-700 dark:hover:bg-seafoam-600 transition-colors
                            disabled:opacity-50 disabled:cursor-not-allowed"
                            disabled=move || {
                                name.get().trim().is_empty() || create_action.pending().get()
                            }

                            on:click=move |_| {
                                create_action.dispatch(());
                            }
                        >

                            {move || {
                                if create_action.pending().get() {
                                    "Creating..."
                                } else {
                                    "Create Project"
                                }
                            }}

                        </button>
                    </div>
                </div>
            </div>
        </div>
    }.into_any()
}

#[component]
fn UploadDocumentModal(
    project_id: Uuid,
    _show: ReadSignal<bool>,
    set_show: WriteSignal<bool>,
    #[prop(into)] on_uploaded: Callback<()>,
) -> impl IntoView {
    let (filename, set_filename) = signal(String::new());
    let (content, set_content) = signal(String::new());
    let (is_uploading, set_is_uploading) = signal(false);
    
    // New state for switching between file upload and manual text input
    let (input_mode, set_input_mode) = signal("file"); // "file" or "text"
    let (manual_filename, set_manual_filename) = signal(String::new());
    let (manual_content, set_manual_content) = signal(String::new());

    let handle_file_upload = move |ev: Event| {
        if let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
            if let Some(files) = input.files() {
                if files.length() > 0 {
                    if let Some(file) = files.get(0) {
                        set_filename.set(file.name());
                        let file_reader = web_sys::FileReader::new().unwrap();
                        let file_reader_clone = file_reader.clone();
                        
                        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
                            if let Ok(result) = file_reader_clone.result() {
                                if let Some(content_str) = result.as_string() {
                                    set_content.set(content_str);
                                }
                            }
                        }) as Box<dyn FnMut(_)>);
                        
                        file_reader.set_onload(Some(closure.as_ref().unchecked_ref()));
                        closure.forget();
                        let _ = file_reader.read_as_text(&file);
                    }
                }
            }
        }
    };

    let upload_action = Action::new(move |_: &()| {
        let project_id = project_id;
        
        // Determine which content to use based on input mode
        let (final_filename, final_content) = if input_mode.get() == "file" {
            (filename.get(), content.get())
        } else {
            let manual_fname = manual_filename.get();
            let fname = if manual_fname.trim().is_empty() {
                "untitled.txt".to_string()
            } else if !manual_fname.contains('.') {
                format!("{}.txt", manual_fname.trim())
            } else {
                manual_fname.trim().to_string()
            };
            (fname, manual_content.get())
        };
        
        async move {
            set_is_uploading.set(true);
            
            match upload_document(
                project_id,
                final_filename,
                final_content,
                Some("text/plain".to_string())
            ).await {
                Ok(_) => {
                    on_uploaded.run(());
                    set_show.set(false);
                    // Reset all form state
                    set_filename.set(String::new());
                    set_content.set(String::new());
                    set_manual_filename.set(String::new());
                    set_manual_content.set(String::new());
                    set_input_mode.set("file");
                    set_is_uploading.set(false);
                    Ok(())
                }
                Err(e) => {
                    set_is_uploading.set(false);
                    Err(format!("Failed to upload document: {}", e))
                }
            }
        }
    });

    // Helper to check if we have valid content to upload
    let has_valid_content = move || {
        if input_mode.get() == "file" {
            !content.get().is_empty()
        } else {
            !manual_content.get().trim().is_empty()
        }
    };

    view! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
            <div class="card-themed p-6 w-full max-w-lg">
                <div class="flex justify-between items-center mb-4">
                    <h3 class="text-lg font-semibold text-themed-primary">"Upload Document"</h3>
                    <IconButton
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Small
                        class="text-themed-secondary hover:text-themed-primary"
                        on_click=Callback::new(move |_| {
                            set_show.set(false);
                            set_filename.set(String::new());
                            set_content.set(String::new());
                            set_manual_filename.set(String::new());
                            set_manual_content.set(String::new());
                            set_input_mode.set("file");
                        })
                    >

                        "✕"
                    </IconButton>
                </div>

                <div class="space-y-4">
                    // Mode selector tabs
                    <div class="flex bg-surface-secondary rounded-lg p-1">
                        <button
                            class=move || {
                                format!(
                                    "flex-1 px-3 py-2 text-sm font-medium rounded-md transition-colors {}",
                                    if input_mode.get() == "file" {
                                        "bg-white dark:bg-teal-700 text-themed-primary shadow-sm"
                                    } else {
                                        "text-themed-secondary hover:text-themed-primary"
                                    },
                                )
                            }

                            on:click=move |_| {
                                set_input_mode.set("file");
                                set_manual_filename.set(String::new());
                                set_manual_content.set(String::new());
                            }
                        >

                            <Icon icon=icondata_bs::BsFolder2Open width="16" height="16"/>
                            "Upload File"
                        </button>
                        <button
                            class=move || {
                                format!(
                                    "flex-1 px-3 py-2 text-sm font-medium rounded-md transition-colors {}",
                                    if input_mode.get() == "text" {
                                        "bg-white dark:bg-teal-700 text-themed-primary shadow-sm"
                                    } else {
                                        "text-themed-secondary hover:text-themed-primary"
                                    },
                                )
                            }

                            on:click=move |_| {
                                set_input_mode.set("text");
                                set_filename.set(String::new());
                                set_content.set(String::new());
                            }
                        >

                            <Icon icon=icondata_bs::BsPencil width="16" height="16"/>
                            "Enter Text"
                        </button>
                    </div>

                    // File upload mode
                    {move || {
                        if input_mode.get() == "file" {
                            view! {
                                <div class="space-y-3">
                                    <div>
                                        <label class="block text-sm font-medium text-themed-primary mb-1">
                                            "Choose File"
                                        </label>
                                        <input
                                            type="file"
                                            accept=".txt,.md,.pdf,.doc,.docx"
                                            class="input-themed w-full"
                                            on:change=handle_file_upload
                                        />
                                        <p class="text-xs text-themed-secondary mt-1">
                                            "Supported formats: .txt, .md, .pdf, .doc, .docx"
                                        </p>
                                    </div>

                                    {move || {
                                        if !filename.get().is_empty() {
                                            view! {
                                                <div class="surface-secondary p-3 rounded">
                                                    <p class="text-sm text-themed-primary">
                                                        "Selected: "
                                                        <span class="font-medium">{filename.get()}</span>
                                                    </p>
                                                    {move || {
                                                        if !content.get().is_empty() {
                                                            view! {
                                                                <p class="text-xs text-themed-secondary mt-1">
                                                                    {format!("{} characters", content.get().len())}
                                                                </p>
                                                            }
                                                                .into_any()
                                                        } else {
                                                            view! { <div></div> }.into_any()
                                                        }
                                                    }}

                                                </div>
                                            }
                                                .into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }
                                    }}

                                </div>
                            }
                                .into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    }}

                    // Manual text input mode
                    {move || {
                        if input_mode.get() == "text" {
                            view! {
                                <div class="space-y-3">
                                    <div>
                                        <label class="block text-sm font-medium text-themed-primary mb-1">
                                            "Document Name"
                                        </label>
                                        <input
                                            type="text"
                                            class="input-themed w-full"
                                            placeholder="Enter document name"
                                            prop:value=manual_filename
                                            on:input=move |ev| {
                                                set_manual_filename.set(event_target_value(&ev))
                                            }
                                        />

                                        <p class="text-xs text-themed-secondary mt-1">
                                            "Enter your document title"
                                        </p>
                                    </div>

                                    <div>
                                        <label class="block text-sm font-medium text-themed-primary mb-1">
                                            "Document Content"
                                        </label>
                                        <textarea
                                            class="input-themed w-full resize-none"
                                            rows="8"
                                            placeholder="Paste or type your document content here..."
                                            prop:value=manual_content
                                            on:input=move |ev| {
                                                set_manual_content.set(event_target_value(&ev))
                                            }
                                        >
                                        </textarea>
                                        <p class="text-xs text-themed-secondary mt-1">
                                            {move || {
                                                let char_count = manual_content.get().len();
                                                if char_count > 0 {
                                                    format!("{} characters", char_count)
                                                } else {
                                                    "Enter your text content".to_string()
                                                }
                                            }}

                                        </p>
                                    </div>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    }}

                    <div class="flex justify-end space-x-3 pt-4">
                        <Button
                            variant=ButtonVariant::Ghost
                            disabled=is_uploading.get()
                            on_click=Callback::new(move |_| {
                                set_show.set(false);
                                set_filename.set(String::new());
                                set_content.set(String::new());
                                set_manual_filename.set(String::new());
                                set_manual_content.set(String::new());
                                set_input_mode.set("file");
                            })
                        >

                            "Cancel"
                        </Button>
                        <button
                            class="px-4 py-2 bg-seafoam-600 dark:bg-seafoam-700 text-gray rounded-md 
                            hover:bg-seafoam-700 dark:hover:bg-seafoam-600 transition-colors
                            disabled:opacity-50 disabled:cursor-not-allowed"
                            disabled=move || !has_valid_content() || is_uploading.get()
                            on:click=move |_| {
                                upload_action.dispatch(());
                            }
                        >

                            {move || {
                                if is_uploading.get() {
                                    "Uploading & Processing..."
                                } else {
                                    "Upload"
                                }
                            }}

                        </button>
                    </div>
                </div>
            </div>
        </div>
    }.into_any()
}
