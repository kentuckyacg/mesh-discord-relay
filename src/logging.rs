use tracing::debug;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

pub fn init(verbose: bool, debug: bool) {

    let should_run_debug = cfg!(debug_assertions) || debug;

    // Figure out if we should run as verbose, debug, or warn
    let directive = match (should_run_debug, verbose) {
        (true, _) => "mesh_discord_relay=DEBUG",
        (_, true) => "mesh_discord_relay=INFO",
        _ => "mesh_discord_relay=WARN"
    };

    // Create the filter
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .from_env().expect("Failed to parse env filter")
        .add_directive(directive.parse().expect("Failed to parse filter"));
    // Setup the subscriber
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    debug!("Successfully setup tracing subscriber.");
}