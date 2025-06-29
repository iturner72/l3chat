use axum::{extract::Request, middleware::Next, response::Response};
use tracing::Instrument;
use tracing_subscriber::fmt::format::Writer;
use tracing::field::Field;
use tracing_subscriber::field::Visit;
use std::fmt;
use uuid::Uuid;

pub async fn trace_requests(request: Request, next: Next) -> Response {
    let request_id = Uuid::new_v4().to_string();

    let span = tracing::info_span!(
        "request",
        method = %request.method(),
        uri = %request.uri(),
        request_id = %request_id,
    );

    async move {
        let response = next.run(request).await;
        tracing::info!("request completed");
        response
    }
    .instrument(span)
    .await
}

// Custom field formatter with your color choices
pub struct ColoredFields;

impl<'writer> tracing_subscriber::fmt::FormatFields<'writer> for ColoredFields {
    fn format_fields<R: tracing_subscriber::field::RecordFields>(
        &self,
        writer: Writer<'writer>,
        fields: R,
    ) -> fmt::Result {
        let mut visitor = ColoredFieldVisitor::new(writer);
        fields.record(&mut visitor);
        visitor.finish()
    }
}

struct ColoredFieldVisitor<'writer> {
    writer: Writer<'writer>,
    is_first: bool,
    error: Option<fmt::Error>,
}

impl<'writer> ColoredFieldVisitor<'writer> {
    fn new(writer: Writer<'writer>) -> Self {
        Self {
            writer,
            is_first: true,
            error: None,
        }
    }

    fn finish(self) -> fmt::Result {
        match self.error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}

impl<'writer> Visit for ColoredFieldVisitor<'writer> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.error.is_some() {
            return; // Don't continue if we already have an error
        }

        let result = (|| {
            if !self.is_first {
                write!(self.writer, " ")?;
            }
            self.is_first = false;

            // Check if ANSI is supported
            if self.writer.has_ansi_escapes() {
                // Custom colors - change these to your preference!
                match field.name() {
                    "request_id" => write!(self.writer, "\x1b[38;2;255;105;180m{}=\x1b[36m{:?}\x1b[0m", field.name(), value)?, // Hot pink key, cyan value
                    "user_id" => write!(self.writer, "\x1b[34m{}=\x1b[31m{:?}\x1b[0m", field.name(), value)?, // Blue key, red value
                    "operation" => write!(self.writer, "\x1b[1;31m{}=\x1b[1;37m{:?}\x1b[0m", field.name(), value)?, // Bold red key, bold white value
                    "project_name" => write!(self.writer, "\x1b[33m{}=\x1b[32m{:?}\x1b[0m", field.name(), value)?, // Yellow key, green value
                    "uri" => write!(self.writer, "\x1b[35m{}=\x1b[1;36m{:?}\x1b[0m", field.name(), value)?, // Purple key, bold green value
                    "method" => write!(self.writer, "\x1b[1;33m{}=\x1b[36m{:?}\x1b[0m", field.name(), value)?, // Bold yellow key, cyan value
                    _ => write!(self.writer, "\x1b[90m{}=\x1b[37m{:?}\x1b[0m", field.name(), value)?, // Gray key, white value for others

                }
            } else {
                write!(self.writer, "{}={:?}", field.name(), value)?;
            }
            Ok(())
        })();

        if let Err(err) = result {
            self.error = Some(err);
        }
    }
}
