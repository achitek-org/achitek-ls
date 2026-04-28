use achitek_ls::{Server, arguments};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};

fn init_logging() {
    let filter = std::env::var("ACHITEK_LOG")
        .ok()
        .and_then(|value| value.parse::<Targets>().ok())
        .unwrap_or_else(|| Targets::new().with_default(LevelFilter::INFO));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .init();
}

fn main() -> anyhow::Result<()> {
    let args = arguments::parse()?;

    init_logging();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        channel = args.channel.as_ref().map(ToString::to_string),
        "starting achitek language server"
    );

    let server = Server::new(args.channel);

    server.run()?;

    tracing::info!("achitek language server stopped");

    Ok(())
}
