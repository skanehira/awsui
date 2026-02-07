use std::fmt::Write;
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

/// エラーのsource chainを展開して詳細メッセージを返す
pub fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        write!(msg, ": {cause}").unwrap();
        source = cause.source();
    }
    msg
}
