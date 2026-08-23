#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    Sink(#[from] curia::SinkError),
    #[error("Internal logger disabled and cannot be acquired or attached")]
    LoggerNotInitialized,
}
