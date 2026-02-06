use crate::aws::model::Instance;
use crate::error::AppError;

/// バックグラウンドタスクからUIスレッドへ送信されるイベント。
#[derive(Debug)]
pub enum AppEvent {
    /// EC2インスタンス一覧の読み込み完了
    InstancesLoaded(Result<Vec<Instance>, AppError>),

    /// EC2アクション（Start/Stop/Reboot）の完了
    ActionCompleted(Result<String, AppError>),
}
