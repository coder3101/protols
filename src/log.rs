use async_lsp::lsp_types::{LogMessageParams, MessageType, TraceValue};
use tokio::sync::mpsc;
use tracing::{
    Level, Subscriber,
    field::{Field, Visit},
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter, Layer, Registry, filter::Directive, layer::SubscriberExt, reload,
    util::SubscriberInitExt,
};

/// A `tracing` layer that forwards log events to an LSP client via a channel.
pub struct ClientLogger {
    /// The sender half of a channel connected to the LSP client's log message handler.
    pub tx: mpsc::Sender<LogMessageParams>,
}

struct MessageVisitor {
    message: String,
    fields: String,
}

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.fields
                .push_str(&format!(" {}={}", field.name(), value));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            if self.message.is_empty() {
                self.message = format!("{:?}", value);
            }
        } else {
            self.fields
                .push_str(&format!(" {}={:?}", field.name(), value));
        }
    }
}

impl<S: Subscriber> Layer<S> for ClientLogger {
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        // Filter: only log events from our crate, or errors from any crate.
        metadata.target().starts_with(env!("CARGO_PKG_NAME"))
            || *metadata.level() <= tracing::Level::ERROR
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = MessageVisitor {
            message: String::with_capacity(256),
            fields: String::with_capacity(256),
        };

        event.record(&mut visitor);

        let metadata = event.metadata();
        let target = metadata.target().split("::").last().unwrap_or("");

        let full_text = if visitor.fields.is_empty() {
            visitor.message
        } else {
            format!("{} | fields:{}", visitor.message, visitor.fields)
        };

        let message = format!("[{}] {}", target, full_text);

        let typ = match *metadata.level() {
            tracing::Level::ERROR => MessageType::ERROR,
            tracing::Level::WARN => MessageType::WARNING,
            tracing::Level::INFO => MessageType::INFO,
            tracing::Level::DEBUG => MessageType::LOG,
            _ => MessageType::LOG,
        };

        let _ = self.tx.try_send(LogMessageParams { typ, message });
    }
}

/// Handle to dynamically reload the log filter at runtime.
pub type LogReloadHandle = reload::Handle<EnvFilter, Registry>;

/// Installs the global tracing subscriber with a reloadable filter, the LSP logger,
/// and a file-based backup logger.
///
/// Returns a tuple containing:
///
/// 1. A [`self::LogReloadHandle`] to dynamically update the log level.
/// 2. A [`WorkerGuard`] that must be kept alive in the main function to ensure
///    non-blocking file logging continues.
pub fn install(tx: mpsc::Sender<LogMessageParams>) -> (LogReloadHandle, WorkerGuard) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    let (filter_layer, reload_handle) = reload::Layer::new(filter);

    let lsp_layer = ClientLogger { tx };

    let dir = std::env::temp_dir();
    eprintln!("file logging at directory: {dir:?}");
    let file_appender = tracing_appender::rolling::daily(dir, "protols.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(lsp_layer)
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking),
        )
        .init();

    (reload_handle, guard)
}

/// Updates the log filter level based on the provided LSP [`TraceValue`].
///
/// Mapping:
///
/// - [`Verbose`] -> `trace`
/// - [`Messages`] -> `debug`
/// - [`Off`] -> `info`
///
/// Note: We use standard `window/logMessage` for all levels. While the LSP spec
/// mentions `$/logTrace`, most clients have better support for `window/logMessage`.
///
/// [`Verbose`]: TraceValue::Verbose
/// [`Messages`]: TraceValue::Messages
/// [`Off`]: TraceValue::Off
pub fn update_level(handle: &LogReloadHandle, value: TraceValue) {
    let pkg_name = env!("CARGO_PKG_NAME");

    let level = match value {
        TraceValue::Verbose => Level::TRACE,
        TraceValue::Messages => Level::DEBUG,
        TraceValue::Off => Level::INFO,
    };

    // Construct directives: "warn" for the whole world, "pkg=level" for us
    let global_directive = Level::WARN.into();
    let pkg_directive = format!("{}={}", pkg_name, level)
        .parse::<Directive>()
        .expect("Failed to parse log directive");

    let filter = EnvFilter::default()
        .add_directive(global_directive)
        .add_directive(pkg_directive);

    if let Err(e) = handle.modify(|f| *f = filter) {
        tracing::error!("failed to reload log filter: {e}");
    } else {
        tracing::info!("logger set to: {level}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_logger_formatting() {
        const TARGET: &str = "test_target";
        const MESSAGE: &str = "initialized session";
        let field_name = "user_id";
        let field_value = 42;

        let (tx, mut rx) = mpsc::channel(10);
        let logger = ClientLogger { tx };
        let subscriber = Registry::default().with(logger);
        let _guard = tracing::subscriber::set_default(subscriber);

        tracing::error!(target: TARGET, user_id = field_value, MESSAGE);

        let msg = rx.try_recv().expect("log message should be received");

        // Assertions for the formatted message
        assert_eq!(msg.typ, MessageType::ERROR);

        assert!(msg.message.contains(MESSAGE));

        let expected_field = format!("{}={}", field_name, field_value);
        assert!(msg.message.contains(&expected_field));

        assert!(msg.message.contains(&format!("[{}]", TARGET)));
    }

    #[test]
    fn test_update_level_mapping() {
        let filter = EnvFilter::new("info");
        let (layer, handle) = reload::Layer::new(filter);
        let _subscriber = Registry::default().with(layer);

        // Ensure that calling update_level for different TraceValues doesn't panic
        // and executes the mapping logic correctly.
        update_level(&handle, TraceValue::Verbose);
        update_level(&handle, TraceValue::Messages);
        update_level(&handle, TraceValue::Off);
    }
}
