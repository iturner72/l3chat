use leptos::prelude::*;
use leptos_fetch::QueryClient;
use leptos_icons::Icon;
use uuid::Uuid;
use web_sys::Event;
use wasm_bindgen::JsCast;

use crate::models::projects::*;
use crate::server_fn::projects::*;

pub async fn get_user_projects_query() -> Result<Vec<ProjectView>, String> {
    get_user_projects().await.map_err(|e| e.to_string())
}

#[component]
pub fn ProjectsPage() -> impl IntoView {
    let client: QueryClient = expect_context();
    let (selected_project, set_selected_project) = signal(None::<Uuid>);
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
                    <button
                        class="px-4 py-2 bg-seafoam-600 dark:bg-seafoam-700 text-white rounded-md 
                        hover:bg-seafoam-700 dark:hover:bg-seafoam-600 transition-colors"
                        on:click=move |_| set_show_create_form.set(true)
                    >
                        "New Project"
                    </button>
                </div>

                <div class="projects-list-container grid grid-cols-1 lg:grid-cols-4 gap-6">
                    <div class="lg:col-span-2">
                        <ProjectsList
                            projects_resource=projects_resource
                            selected_project=selected_project
                            set_selected_project=set_selected_project
                        />
                    </div>

                    <div class="project-details lg:col-span-2">
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
    }
}

#[component]
pub fn ProjectsList(
    projects_resource: Resource<Result<Vec<ProjectView>, String>>,
    selected_project: ReadSignal<Option<Uuid>>,
    set_selected_project: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    view! {
        <div class="bg-white dark:bg-teal-800 rounded-lg shadow-md p-6">
            <h2 class="text-xl font-semibold text-gray-800 dark:text-gray-200 mb-4">
                "Your Projects"
            </h2>

            <Transition fallback=|| {
                view! {
                    <div class="space-y-3">
                        <div class="animate-pulse bg-gray-200 dark:bg-teal-600 h-16 rounded-md"></div>
                        <div class="animate-pulse bg-gray-200 dark:bg-teal-600 h-16 rounded-md"></div>
                        <div class="animate-pulse bg-gray-200 dark:bg-teal-600 h-16 rounded-md"></div>
                    </div>
                }
            }>
                {move || {
                    match projects_resource.get() {
                        Some(Ok(projects)) => {
                            if projects.is_empty() {
                                view! {
                                    <div class="text-center text-gray-500 dark:text-gray-400 py-8">
                                        <div class="text-2xl mb-2">"📋"</div>
                                        <p class="text-sm">
                                            "No projects yet. Create your first project to get started!"
                                        </p>
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="space-y-2">
                                        <For
                                            each=move || projects.clone()
                                            key=|project| project.id
                                            children=move |project| {
                                                let project_id = project.id;
                                                let is_selected = move || {
                                                    selected_project.get() == Some(project_id)
                                                };
                                                view! {
                                                    <div class="flex flex-row items-center justify-between space-x-2">
                                                        <button
                                                            class=move || {
                                                                format!(
                                                                    "group w-full text-left p-4 rounded-md border-2 transition-colors {}",
                                                                    if is_selected() {
                                                                        "border-seafoam-500 bg-seafoam-50 dark:bg-seafoam-900/20"
                                                                    } else {
                                                                        "border-gray-200 dark:border-teal-600 hover:border-seafoam-300 dark:hover:border-seafoam-600"
                                                                    },
                                                                )
                                                            }

                                                            on:click=move |_| set_selected_project.set(Some(project_id))
                                                        >
                                                            <div class="flex flex-row items-center justify-between gap-2">
                                                                <div class="flex-1 min-w-0">
                                                                    <h3 class="font-medium text-gray-800 dark:text-gray-200">
                                                                        {project.name.clone()}
                                                                    </h3>
                                                                    {project
                                                                        .description
                                                                        .as_ref()
                                                                        .map(|desc| {
                                                                            view! {
                                                                                <p class="text-sm text-gray-600 dark:text-gray-400 mt-1 truncate">
                                                                                    {desc.clone()}
                                                                                </p>
                                                                            }
                                                                        })}

                                                                </div>
                                                            </div>
                                                        </button>
                                                        <DeleteProjectButton
                                                            project_id=project_id
                                                            project_name=project.name.clone()
                                                        />
                                                    </div>
                                                }
                                            }
                                        />

                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Some(Err(e)) => {
                            view! {
                                <div class="text-center text-salmon-500 py-8">
                                    <p>"Error loading projects: " {e}</p>
                                </div>
                            }
                                .into_any()
                        }
                        None => view! { <div></div> }.into_any(),
                    }
                }}

            </Transition>
        </div>
    }
}

#[component]
pub fn DeleteProjectButton(
    project_id: Uuid,
    project_name: String,
) -> impl IntoView {
    let client: QueryClient = expect_context();
    let (show_confirm, set_show_confirm) = signal(false);

    let delete_action = Action::new(move |&project_id: &Uuid| {
        async move {
            match delete_project(project_id).await {
                Ok(_) => {
                    client.invalidate_query(get_user_projects_query, ());
                    Ok(())
                }
                Err(e) => Err(format!("Failed to delete project: {}", e))
            }
        }
    });

    view! {
        <div class="relative">
            <button
                class="opacity-80 group-hover:opacity-100 transition-opacity p-1 text-salmon-500 hover:text-salmon-700 dark:text-salmon-400 dark:hover:text-salmon-300"
                on:click=move |ev| {
                    ev.stop_propagation();
                    set_show_confirm.set(true);
                }

                title="Delete project"
            >
                {move || {
                    view! { <Icon icon=icondata::BsTrash3 width="16" height="16"/> }
                }}

            </button>

            {move || {
                if show_confirm.get() {
                    view! {
                        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
                            <div class="bg-white dark:bg-teal-800 rounded-lg shadow-xl p-6 w-full max-w-md">
                                <div class="flex items-center mb-4">
                                    <div class="text-salmon-500 text-2xl mr-3">"⚠️"</div>
                                    <h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200">
                                        "Delete Project"
                                    </h3>
                                </div>

                                <div class="mb-6">
                                    <p class="text-gray-700 dark:text-gray-300 mb-2">
                                        "Are you sure you want to delete the project:"
                                    </p>
                                    <p class="font-medium text-gray-900 dark:text-gray-100 bg-gray-100 dark:bg-teal-700 p-2 rounded">
                                        "\"" {project_name.clone()} "\""
                                    </p>
                                    <p class="text-sm text-salmon-600 dark:text-salmon-400 mt-3">
                                        "This will permanently delete:"
                                    </p>
                                    <ul class="text-sm text-salmon-600 dark:text-salmon-400 mt-1 ml-4">
                                        <li>"• All project documents and embeddings"</li>
                                        <li>"• All chat threads associated with this project"</li>
                                        <li>"• All messages in those threads"</li>
                                    </ul>
                                    <p class="ib text-sm font-medium text-salmon-700 dark:text-salmon-300 mt-2">
                                        "This action cannot be undone."
                                    </p>
                                </div>

                                <div class="flex justify-end space-x-3">
                                    <button
                                        class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
                                        on:click=move |_| set_show_confirm.set(false)
                                        disabled=move || delete_action.pending().get()
                                    >
                                        "Cancel"
                                    </button>
                                    <button
                                        class="px-4 py-2 bg-salmon-600 dark:bg-salmon-700 text-white rounded-md 
                                        hover:bg-salmon-700 dark:hover:bg-salmon-600 transition-colors
                                        disabled:opacity-50 disabled:cursor-not-allowed"
                                        disabled=move || delete_action.pending().get()
                                        on:click=move |_| {
                                            delete_action.dispatch(project_id);
                                            set_show_confirm.set(false);
                                        }
                                    >

                                        {move || {
                                            if delete_action.pending().get() {
                                                "Deleting..."
                                            } else {
                                                "Delete Project"
                                            }
                                        }}

                                    </button>
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
    }
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
        <div class="space-y-6">
            <div class="project-actions bg-white dark:bg-teal-800 rounded-lg shadow-md p-6">
                <div class="flex justify-between items-center mb-4">
                    <h2 class="text-xl font-semibold text-gray-800 dark:text-gray-200">
                        "Project Documents"
                    </h2>
                    <div class="flex space-x-2">
                        <button
                            class="px-4 py-2 bg-seafoam-600 dark:bg-seafoam-700 text-gray rounded-md 
                            hover:bg-seafoam-700 dark:hover:bg-seafoam-600 transition-colors"
                            on:click=move |_| set_show_upload.set(true)
                        >
                            "Upload Document"
                        </button>
                        <CreateProjectThreadButton project_id=project_id/>
                    </div>
                </div>

                <Transition fallback=|| {
                    view! {
                        <div class="documents-list space-y-3">
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-600 h-12 rounded-md"></div>
                            <div class="animate-pulse bg-gray-200 dark:bg-teal-600 h-12 rounded-md"></div>
                        </div>
                    }
                }>
                    {move || {
                        match documents_resource.get() {
                            Some(Ok(documents)) => {
                                if documents.is_empty() {
                                    view! {
                                        <div class="text-center text-gray-500 dark:text-gray-400 py-8">
                                            <div class="text-2xl mb-2">"📄"</div>
                                            <p class="text-sm">
                                                "No documents uploaded yet. Upload your first document to enable RAG chat!"
                                            </p>
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="space-y-2">
                                            <For
                                                each=move || documents.clone()
                                                key=|doc| doc.id
                                                children=move |doc| {
                                                    view! {
                                                        <div class="flex items-center justify-between p-3 border border-gray-200 dark:border-teal-600 rounded-md">
                                                            <div class="flex items-center space-x-3">
                                                                <div class="text-2xl">"📄"</div>
                                                                <div>
                                                                    <p class="font-medium text-gray-800 dark:text-gray-200">
                                                                        {doc.filename}
                                                                    </p>
                                                                    <p class="text-sm text-gray-600 dark:text-gray-400">
                                                                        {format!("{} bytes", doc.file_size.unwrap_or(0))}
                                                                    </p>
                                                                </div>
                                                            </div>
                                                            <div class="text-sm text-gray-500 dark:text-gray-400">
                                                                {doc
                                                                    .created_at
                                                                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                                                    .unwrap_or_default()}
                                                            </div>
                                                        </div>
                                                    }
                                                }
                                            />

                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Some(Err(e)) => {
                                view! {
                                    <div class="text-center text-salmon-500 py-4">
                                        <p>"Error loading documents: " {e}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                            None => view! { <div></div> }.into_any(),
                        }
                    }}

                </Transition>
            </div>

            {move || {
                if show_upload.get() {
                    view! {
                        <UploadDocumentModal
                            project_id=project_id
                            _show=show_upload
                            set_show=set_show_upload
                            on_uploaded=move || {
                                documents_resource.refetch();
                            }
                        />
                    }
                        .into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

        </div>
    }
}

#[component]
fn CreateProjectThreadButton(project_id: Uuid) -> impl IntoView {
    let create_thread_action = Action::new(move |&project_id: &Uuid| {
        async move {
            match create_project_thread(project_id).await {
                Ok(thread_id) => {
                    // TODO fix this nav, need threads and project routes
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().set_href(&format!("/?thread={}", thread_id));
                    }
                    Ok(())
                }
                Err(e) => Err(format!("Failed to create thread: {}", e))
            }
        }
    });

    view! {
        <button
            class="px-4 py-2 bg-seafoam-600 dark:bg-seafoam-700 text-white rounded-md 
            hover:bg-seafoam-700 dark:hover:bg-seafoam-600 transition-colors
            disabled:opacity-50 disabled:cursor-not-allowed"
            disabled=move || create_thread_action.pending().get()
            on:click=move |_| {
                create_thread_action.dispatch(project_id);
            }
        >

            {move || {
                if create_thread_action.pending().get() { "Creating Chat..." } else { "Start Chat" }
            }}

        </button>
    }
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
            <div class="bg-white dark:bg-teal-800 rounded-lg shadow-xl p-6 w-full max-w-md">
                <div class="flex justify-between items-center mb-4">
                    <h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200">
                        "Create New Project"
                    </h3>
                    <button
                        class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                        on:click=move |_| set_show.set(false)
                    >
                        "✕"
                    </button>
                </div>

                <div class="space-y-4">
                    <div>
                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            "Project Name"
                        </label>
                        <input
                            type="text"
                            class="w-full px-3 py-2 border border-gray-300 dark:border-teal-600 rounded-md 
                            bg-white dark:bg-teal-700 text-gray-800 dark:text-gray-200"
                            placeholder="Enter project name"
                            prop:value=name
                            on:input=move |ev| set_name.set(event_target_value(&ev))
                        />
                    </div>

                    <div>
                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            "Description (Optional)"
                        </label>
                        <textarea
                            class="w-full px-3 py-2 border border-gray-300 dark:border-teal-600 rounded-md 
                            bg-white dark:bg-teal-700 text-gray-800 dark:text-gray-200"
                            rows="3"
                            placeholder="Brief description of the project"
                            prop:value=description
                            on:input=move |ev| set_description.set(event_target_value(&ev))
                        ></textarea>
                    </div>

                    <div>
                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            "AI Instructions (Optional)"
                        </label>
                        <textarea
                            class="w-full px-3 py-2 border border-gray-300 dark:border-teal-600 rounded-md 
                            bg-white dark:bg-teal-700 text-gray-800 dark:text-gray-200"
                            rows="4"
                            placeholder="Instructions for how the AI should behave in this project context"
                            prop:value=instructions
                            on:input=move |ev| set_instructions.set(event_target_value(&ev))
                        ></textarea>
                    </div>

                    <div class="flex justify-end space-x-2 pt-4">
                        <button
                            class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
                            on:click=move |_| set_show.set(false)
                            disabled=move || create_action.pending().get()
                        >
                            "Cancel"
                        </button>
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
    }
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

    let handle_file_upload = move |ev: Event| {
        if let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
            if let Some(files) = input.files() {
                if files.length() > 0 {
                    if let Some(file) = files.get(0) {
                        set_filename.set(file.name());
                        
                        let file_reader = web_sys::FileReader::new().unwrap();
                        let file_reader_clone = file_reader.clone();
                        
                        let on_load = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: Event| {
                            if let Ok(result) = file_reader_clone.result() {
                                if let Some(text) = result.as_string() {
                                    set_content.set(text);
                                }
                            }
                        }) as Box<dyn FnMut(_)>);
                        
                        file_reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
                        on_load.forget();
                        
                        let _ = file_reader.read_as_text(&file);
                    }
                }
            }
        }
    };

    let upload_action = Action::new(move |_: &()| {
        let filename_val = filename.get();
        let content_val = content.get();
        
        async move {
            set_is_uploading.set(true);
            
            let result = upload_document(
                project_id,
                filename_val,
                content_val,
                Some("text/plain".to_string())
            ).await;
            
            set_is_uploading.set(false);
            
            match result {
                Ok(_) => {
                    set_show.set(false);
                    set_filename.set(String::new());
                    set_content.set(String::new());
                    on_uploaded.run(());
                    Ok(())
                }
                Err(e) => Err(format!("Failed to upload document: {}", e))
            }
        }
    });

    view! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
            <div class="bg-white dark:bg-teal-800 rounded-lg shadow-xl p-6 w-full max-w-lg">
                <div class="flex justify-between items-center mb-4">
                    <h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200">
                        "Upload Document"
                    </h3>
                    <button
                        class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                        on:click=move |_| set_show.set(false)
                    >
                        "✕"
                    </button>
                </div>

                <div class="space-y-4">
                    <div>
                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            "Select File"
                        </label>
                        <input
                            type="file"
                            accept=".txt,.md,.text"
                            class="w-full px-3 py-2 border border-gray-300 dark:border-teal-600 rounded-md 
                            bg-white dark:bg-teal-700 text-gray-800 dark:text-gray-200"
                            on:change=handle_file_upload
                        />
                        <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
                            "Supported formats: .txt, .md, .text"
                        </p>
                    </div>

                    {move || {
                        if !filename.get().is_empty() {
                            view! {
                                <div>
                                    <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        "Preview"
                                    </label>
                                    <div class="p-3 border border-gray-300 dark:border-teal-600 rounded-md bg-gray-50 dark:bg-teal-700">
                                        <p class="text-sm font-medium text-gray-800 dark:text-gray-200 mb-2">
                                            {filename.get()}
                                        </p>
                                        <div class="text-xs text-gray-600 dark:text-gray-400 max-h-32 overflow-y-auto">
                                            {content.get().chars().take(500).collect::<String>()}
                                            {move || if content.get().len() > 500 { "..." } else { "" }}
                                        </div>
                                    </div>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    }}

                    <div class="flex justify-end space-x-2 pt-4">
                        <button
                            class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
                            on:click=move |_| set_show.set(false)
                        >
                            "Cancel"
                        </button>
                        <button
                            class="px-4 py-2 bg-seafoam-600 dark:bg-seafoam-700 text-gray rounded-md 
                            hover:bg-seafoam-700 dark:hover:bg-seafoam-600 transition-colors
                            disabled:opacity-50 disabled:cursor-not-allowed"
                            disabled=move || content.get().is_empty() || is_uploading.get()
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
    }
}
