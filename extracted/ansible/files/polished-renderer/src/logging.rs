use tracing_subscriber::EnvFilter;

pub fn init_logging() {
    if tracing::dispatcher::has_been_set() {
        return;
    }

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,polished_renderer=info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init();
}
