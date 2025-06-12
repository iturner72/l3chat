use leptos::prelude::*;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::fmt::Write;

#[component]
pub fn MarkdownRenderer(
    #[prop(into)] content: String,
    #[prop(optional)] class: &'static str,
) -> impl IntoView {
    let rendered_html = Memo::new(move |_| markdown_to_html(&content));

    view! {
        <div
            class=format!("markdown-content {} min-w-0 max-w-full overflow-hidden", class)
            inner_html=move || rendered_html.get()
        ></div>
    }
}

pub fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    let mut in_code_block = false;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                in_code_block = true;
                let code_lang = lang.to_string();
                let lang_display = if code_lang.is_empty() {
                    "text".to_string()
                } else {
                    html_escape(&code_lang)
                };
                let lang_class = html_escape(&code_lang);
                write!(
                    html_output,
                    r#"<div class="relative my-4 min-w-0 max-w-full">
                        <div class="flex items-center justify-between bg-gray-200 dark:bg-teal-900 px-4 py-2 text-xs text-gray-600 dark:text-gray-400 rounded-t-lg border-b border-gray-300 dark:border-teal-700">
                            <span class="font-medium">{}</span>
                            <button onclick="navigator.clipboard.writeText(this.parentElement.nextElementSibling.textContent)" 
                                    class="hover:text-gray-800 dark:hover:text-gray-200 transition-colors">
                                Copy
                            </button>
                        </div>
                        <pre class="bg-gray-100 dark:bg-teal-900 rounded-b-lg p-4 overflow-x-auto text-left min-w-0 max-w-full"><code class="language-{} text-sm font-mono block whitespace-pre text-left min-w-0 max-w-full">"#,
                    lang_display,
                    lang_class
                ).unwrap();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                html_output.push_str("</code></pre></div>");
            }
            Event::Start(Tag::Paragraph) => {
                if !in_code_block {
                    html_output.push_str(r#"<p class="mb-4 leading-relaxed text-left">"#);
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_code_block {
                    html_output.push_str("</p>");
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                let size_class = match level {
                    pulldown_cmark::HeadingLevel::H1 => "text-2xl font-bold mb-4 mt-6 text-gray-900 dark:text-gray-100",
                    pulldown_cmark::HeadingLevel::H2 => "text-xl font-semibold mb-3 mt-5 text-gray-900 dark:text-gray-100",
                    pulldown_cmark::HeadingLevel::H3 => "text-lg font-medium mb-2 mt-4 text-gray-900 dark:text-gray-100",
                    _ => "text-base font-medium mb-2 mt-3 text-gray-900 dark:text-gray-100",
                };
                write!(html_output, r#"<h{} class="{}">"#, level as u8, size_class).unwrap();
            }
            Event::End(TagEnd::Heading(level)) => {
                write!(html_output, "</h{}>", level as u8).unwrap();
            }
            Event::Start(Tag::Strong) => {
                html_output
                    .push_str(r#"<strong class="font-semibold text-gray-900 dark:text-gray-100">"#);
            }
            Event::End(TagEnd::Strong) => {
                html_output.push_str("</strong>");
            }
            Event::Start(Tag::Emphasis) => {
                html_output.push_str(r#"<em class="italic text-gray-800 dark:text-gray-200">"#);
            }
            Event::End(TagEnd::Emphasis) => {
                html_output.push_str("</em>");
            }
            Event::Start(Tag::Link {
                dest_url, title, ..
            }) => {
                write!(
                    html_output,
                    r#"<a href="{}" title="{}" class="text-seafoam-600 dark:text-seafoam-400 hover:text-seafoam-700 dark:hover:text-seafoam-300 underline" target="_blank" rel="noopener noreferrer">"#,
                    html_escape(&dest_url),
                    html_escape(&title)
                ).unwrap();
            }
            Event::End(TagEnd::Link) => {
                html_output.push_str("</a>");
            }
            Event::Start(Tag::List(None)) => {
                html_output.push_str(r#"<ul class="list-disc list-inside mb-4 ml-4 space-y-1 text-left">"#);
            }
            Event::Start(Tag::List(Some(_))) => {
                html_output
                    .push_str(r#"<ol class="list-decimal list-inside mb-4 ml-4 space-y-1 text-left">"#);
            }
            Event::End(TagEnd::List(false)) => {
                html_output.push_str("</ul>");
            }
            Event::End(TagEnd::List(true)) => {
                html_output.push_str("</ol>");
            }
            Event::Start(Tag::Item) => {
                html_output.push_str(r#"<li class="text-gray-800 dark:text-gray-200">"#);
            }
            Event::End(TagEnd::Item) => {
                html_output.push_str("</li>");
            }
            Event::Start(Tag::BlockQuote(_)) => {
                html_output.push_str(r#"<blockquote class="border-l-4 border-gray-300 dark:border-teal-600 pl-4 py-2 my-4 italic text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-teal-800/50 rounded-r-lg text-left">"#);
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                html_output.push_str("</blockquote>");
            }
            Event::Code(text) => {
                write!(
                    html_output,
                    r#"<code class="bg-gray-200 dark:bg-teal-700 px-1.5 py-0.5 rounded text-sm font-mono text-gray-800 dark:text-gray-200">{}</code>"#,
                    html_escape(&text)
                ).unwrap();
            }
            Event::Text(text) => {
                if in_code_block {
                    // Preserve formatting and indentation in code blocks
                    html_output.push_str(&html_escape(&text));
                } else {
                    html_output.push_str(&html_escape(&text));
                }
            }
            Event::SoftBreak => {
                if in_code_block {
                    html_output.push('\n');
                } else {
                    html_output.push(' ');
                }
            }
            Event::HardBreak => {
                if in_code_block {
                    html_output.push('\n');
                } else {
                    html_output.push_str("<br>");
                }
            }
            Event::Start(Tag::Table(_)) => {
                html_output.push_str(r#"<div class="overflow-x-auto my-4 w-full max-w-full"><table class="min-w-full border border-gray-300 dark:border-teal-600 bg-white dark:bg-teal-800 w-full max-w-full">"#);
            }
            Event::End(TagEnd::Table) => {
                html_output.push_str("</table></div>");
            }
            Event::Start(Tag::TableHead) => {
                html_output.push_str(r#"<thead class="bg-gray-100 dark:bg-teal-700">"#);
            }
            Event::End(TagEnd::TableHead) => {
                html_output.push_str("</thead>");
            }
            Event::Start(Tag::TableRow) => {
                html_output.push_str("<tr>");
            }
            Event::End(TagEnd::TableRow) => {
                html_output.push_str("</tr>");
            }
            Event::Start(Tag::TableCell) => {
                html_output.push_str(
                    r#"<td class="border border-gray-300 dark:border-teal-600 px-3 py-2 text-gray-800 dark:text-gray-200">"#,
                );
            }
            Event::End(TagEnd::TableCell) => {
                html_output.push_str("</td>");
            }
            Event::Start(Tag::Strikethrough) => {
                html_output.push_str(r#"<del class="line-through text-gray-600 dark:text-gray-400">"#);
            }
            Event::End(TagEnd::Strikethrough) => {
                html_output.push_str("</del>");
            }
            _ => {}
        }
    }

    html_output
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let markdown = "# Hello\n\nThis is **bold** and *italic* text.";
        let html = markdown_to_html(markdown);
        assert!(html.contains("<h1"));
        assert!(html.contains("<strong"));
        assert!(html.contains("<em"));
    }

    #[test]
    fn test_code_blocks() {
        let markdown = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let html = markdown_to_html(markdown);
        assert!(html.contains("<pre"));
        assert!(html.contains("language-rust"));
        assert!(html.contains("Copy"));
    }

    #[test]
    fn test_code_indentation() {
        let markdown = "```python\ndef hello():\n    print(\"Hello\")\n    if True:\n        print(\"Indented\")\n```";
        let html = markdown_to_html(markdown);
        assert!(html.contains("    print"));
        assert!(html.contains("        print"));
    }
}
