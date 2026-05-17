use achitek_ls::{arguments, language_server};

fn main() -> anyhow::Result<()> {
    let args = arguments::parse()?;
    if let Err(error) = language_server::init_logging(args.log_file.clone()) {
        eprintln!("failed to initialize logging: {error:#}");
    }
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        channel = args.channel.as_ref().map(ToString::to_string),
        "starting achitek language server"
    );
    language_server::run(args.channel)?;
    tracing::info!("achitek language server stopped");

    Ok(())
}
