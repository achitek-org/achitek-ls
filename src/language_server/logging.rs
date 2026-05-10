use anyhow::Context;
use std::{path::PathBuf, sync::Arc};
use tracing_subscriber::{
    Layer, filter::Targets, fmt::writer::BoxMakeWriter, layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub fn init_logging(log_file: Option<PathBuf>) -> anyhow::Result<()> {
    let filter = std::env::var("ACHITEK_LOG").unwrap_or_else(|_| "warn".to_owned());
    let filter = filter
        .parse::<Targets>()
        .with_context(|| format!("invalid ACHITEK_LOG filter `{filter}`"))?;
    let log_file = std::env::var("ACHITEK_LOG_FILE")
        .ok()
        .map(PathBuf::from)
        .or(log_file);
    let writer = match log_file {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create log directory `{}`", parent.display())
                })?;
            }
            let file = std::fs::File::create(&path)
                .with_context(|| format!("failed to create log file `{}`", path.display()))?;
            BoxMakeWriter::new(Arc::new(file))
        }
        None => BoxMakeWriter::new(std::io::stderr),
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false)
                .with_target(false)
                .with_filter(filter),
        )
        .init();

    Ok(())
}
