use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("AWS API error: {0}")]
    AwsApi(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
