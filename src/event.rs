use crate::aws::ecr_model::{Image, Repository};
use crate::aws::ecs_model::{Cluster, ContainerLogConfig, Service, Task};
use crate::aws::logs_model::LogEvent;
use crate::aws::model::Instance;
use crate::aws::s3_model::{Bucket, S3Object};
use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::aws::vpc_model::{Subnet, Vpc};
use crate::error::AppError;
use crate::tab::TabId;

/// タブ固有のイベント
#[derive(Debug)]
pub enum TabEvent {
    /// EC2インスタンス一覧の読み込み完了
    InstancesLoaded(Result<Vec<Instance>, AppError>),

    /// EC2アクション（Start/Stop/Reboot）の完了
    ActionCompleted(Result<String, AppError>),

    /// ECRリポジトリ一覧の読み込み完了
    RepositoriesLoaded(Result<Vec<Repository>, AppError>),

    /// ECRイメージ一覧の読み込み完了
    ImagesLoaded(Result<Vec<Image>, AppError>),

    /// ECSクラスター一覧の読み込み完了
    ClustersLoaded(Result<Vec<Cluster>, AppError>),

    /// ECSサービス一覧の読み込み完了
    EcsServicesLoaded(Result<Vec<Service>, AppError>),

    /// ECSタスク一覧の読み込み完了
    EcsTasksLoaded(Result<Vec<Task>, AppError>),

    /// S3バケット一覧の読み込み完了
    BucketsLoaded(Result<Vec<Bucket>, AppError>),

    /// S3オブジェクト一覧の読み込み完了
    ObjectsLoaded(Result<Vec<S3Object>, AppError>),

    /// VPC一覧の読み込み完了
    VpcsLoaded(Result<Vec<Vpc>, AppError>),

    /// サブネット一覧の読み込み完了
    SubnetsLoaded(Result<Vec<Subnet>, AppError>),

    /// シークレット一覧の読み込み完了
    SecretsLoaded(Result<Vec<Secret>, AppError>),

    /// シークレット詳細の読み込み完了
    SecretDetailLoaded(Result<Box<SecretDetail>, AppError>),

    /// シークレット値の取得完了
    SecretValueLoaded(Result<String, AppError>),

    /// ナビゲーションリンク先のVPCデータ読み込み完了
    NavigateVpcLoaded(Result<(Vec<Vpc>, Vec<Subnet>), AppError>),

    /// ECSタスク定義のログ設定読み込み完了
    EcsLogConfigsLoaded(Result<Vec<ContainerLogConfig>, AppError>),

    /// ECSログイベント読み込み完了
    EcsLogEventsLoaded(Result<(Vec<LogEvent>, Option<String>), AppError>),
}

/// バックグラウンドタスクからUIスレッドへ送信されるイベント。
#[derive(Debug)]
pub enum AppEvent {
    /// タブ固有のイベント
    TabEvent(TabId, TabEvent),

    /// CRUD操作の完了（汎用：成功メッセージまたはエラー）
    CrudCompleted(TabId, Result<String, AppError>),
}
