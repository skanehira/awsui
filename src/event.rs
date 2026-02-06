use crate::aws::ecr_model::{Image, Repository};
use crate::aws::ecs_model::{Cluster, Service};
use crate::aws::model::Instance;
use crate::aws::s3_model::{Bucket, S3Object};
use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::aws::vpc_model::{Subnet, Vpc};
use crate::error::AppError;

/// バックグラウンドタスクからUIスレッドへ送信されるイベント。
#[derive(Debug)]
pub enum AppEvent {
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
}
