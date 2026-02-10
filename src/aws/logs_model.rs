/// CloudWatch Logsのログイベント
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    pub timestamp: i64,
    pub formatted_time: String,
    pub message: String,
}
