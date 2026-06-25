use std::{io::{self, IsTerminal}, panic, sync::OnceLock};

use tracing_subscriber::{fmt, EnvFilter};

static LOG_GUARD: OnceLock<()> = OnceLock::new();

/// Default filter when neither `KAISHA_LOG` nor `RUST_LOG` is set.
pub fn default_env_filter() -> &'static str {
    "info,server=debug,hyper=warn,tower=warn,axum=warn"
}

/// Resolve the tracing filter directive from env vars.
///
/// Precedence: `KAISHA_LOG` > `RUST_LOG` > [`default_env_filter`].
pub fn resolve_log_filter(kaisha_log: Option<&str>, rust_log: Option<&str>) -> String {
    kaisha_log
        .or(rust_log)
        .unwrap_or(default_env_filter())
        .to_string()
}

fn build_env_filter() -> EnvFilter {
    let kaisha_log = std::env::var("KAISHA_LOG").ok();
    let rust_log = std::env::var("RUST_LOG").ok();
    let spec = resolve_log_filter(kaisha_log.as_deref(), rust_log.as_deref());
    EnvFilter::try_new(&spec).unwrap_or_else(|_| EnvFilter::new(default_env_filter()))
}

/// Initialize structured logging to stdout. Safe to call multiple times; only the first call installs the subscriber.
pub fn init() {
    LOG_GUARD.get_or_init(|| {
        let filter = build_env_filter();
        fmt()
            .with_writer(io::stdout)
            .with_ansi(io::stdout().is_terminal())
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_env_filter(filter)
            .init();
        install_panic_hook();
    });
}

/// Log panics through tracing so they appear on stdout with the rest of the log stream.
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| {
                info.payload()
                    .downcast_ref::<String>()
                    .map(String::as_str)
            })
            .unwrap_or("<non-string panic payload>");

        if let Some(location) = info.location() {
            tracing::error!(
                panic.payload = payload,
                panic.file = location.file(),
                panic.line = location.line(),
                panic.column = location.column(),
                "unhandled panic"
            );
        } else {
            tracing::error!(panic.payload = payload, "unhandled panic");
        }

        default_hook(info);
    }));
}

pub fn http_trace_layer(
) -> tower_http::trace::TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
> {
    use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, TraceLayer};
    use tracing::Level;

    TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_failure(DefaultOnFailure::new().level(Level::ERROR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_env_filter_targets_server_debug() {
        assert!(default_env_filter().contains("server=debug"));
    }

    #[test]
    fn resolve_log_filter_prefers_kaisha_log() {
        assert_eq!(
            resolve_log_filter(Some("debug"), Some("warn")),
            "debug"
        );
    }

    #[test]
    fn resolve_log_filter_falls_back_to_rust_log() {
        assert_eq!(resolve_log_filter(None, Some("warn")), "warn");
    }

    #[test]
    fn resolve_log_filter_uses_default_when_unset() {
        assert_eq!(resolve_log_filter(None, None), default_env_filter());
    }
}
