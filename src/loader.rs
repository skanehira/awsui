use std::sync::Arc;

use awsui::app::App;
use awsui::aws::client::Ec2Client;
use awsui::aws::s3_client::S3Client;
use awsui::event::{AppEvent, TabEvent};
use awsui::service::ServiceKind;
use awsui::tab::{TabId, TabView};
use tokio::sync::mpsc;

use crate::clients::Clients;

pub(crate) fn load_instances(
    tx: &mpsc::Sender<AppEvent>,
    client: Arc<dyn Ec2Client>,
    tab_id: TabId,
) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let result = client.describe_instances().await;
        let _ = tx
            .send(AppEvent::TabEvent(
                tab_id,
                TabEvent::InstancesLoaded(result),
            ))
            .await;
    });
}

/// S3 list_objects を非同期実行する共通ヘルパー
fn spawn_list_objects(
    tx: &mpsc::Sender<AppEvent>,
    client: &Arc<dyn S3Client>,
    selected_bucket: &Option<String>,
    current_prefix: &str,
    tab_id: TabId,
) {
    let tx = tx.clone();
    let c = client.clone();
    let bucket = selected_bucket.clone().unwrap_or_default();
    let prefix = if current_prefix.is_empty() {
        None
    } else {
        Some(current_prefix.to_string())
    };
    tokio::spawn(async move {
        let result = c.list_objects(&bucket, prefix).await;
        let _ = tx
            .send(AppEvent::TabEvent(tab_id, TabEvent::ObjectsLoaded(result)))
            .await;
    });
}

pub(crate) fn load_detail_data(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };

    match &tab.data {
        awsui::tab::ServiceData::Ecr { repositories, .. } => {
            if let Some(repo) = repositories.filtered.get(tab.selected_index)
                && let Some(client) = &clients.ecr
            {
                let tx = app.event_tx.clone();
                let c = client.clone();
                let repo_name = repo.repository_name.clone();
                tokio::spawn(async move {
                    let result = c.list_images(&repo_name).await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::ImagesLoaded(result)))
                        .await;
                });
            }
        }
        awsui::tab::ServiceData::Ecs { clusters, .. } => {
            if let Some(cluster) = clusters.filtered.get(tab.selected_index)
                && let Some(client) = &clients.ecs
            {
                let tx = app.event_tx.clone();
                let c = client.clone();
                let cluster_arn = cluster.cluster_arn.clone();
                tokio::spawn(async move {
                    let result = c.list_services(&cluster_arn).await;
                    let _ = tx
                        .send(AppEvent::TabEvent(
                            tab_id,
                            TabEvent::EcsServicesLoaded(result),
                        ))
                        .await;
                });
            }
        }
        awsui::tab::ServiceData::S3 {
            selected_bucket,
            current_prefix,
            ..
        } => {
            if let Some(client) = &clients.s3 {
                spawn_list_objects(
                    &app.event_tx,
                    client,
                    selected_bucket,
                    current_prefix,
                    tab_id,
                );
            }
        }
        awsui::tab::ServiceData::Vpc { vpcs, .. } => {
            if let Some(vpc) = vpcs.filtered.get(tab.selected_index)
                && let Some(client) = &clients.vpc
            {
                let tx = app.event_tx.clone();
                let c = client.clone();
                let vpc_id = vpc.vpc_id.clone();
                tokio::spawn(async move {
                    let result = c.describe_subnets(&vpc_id).await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::SubnetsLoaded(result)))
                        .await;
                });
            }
        }
        awsui::tab::ServiceData::Secrets { secrets, .. } => {
            if let Some(secret) = secrets.filtered.get(tab.selected_index)
                && let Some(client) = &clients.secrets
            {
                let tx = app.event_tx.clone();
                let c = client.clone();
                let secret_id = secret.arn.clone();
                tokio::spawn(async move {
                    let result = c.describe_secret(&secret_id).await;
                    let _ = tx
                        .send(AppEvent::TabEvent(
                            tab_id,
                            TabEvent::SecretDetailLoaded(result.map(Box::new)),
                        ))
                        .await;
                });
            }
        }
        _ => {}
    }
}

pub(crate) fn load_secret_value(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    if let awsui::tab::ServiceData::Secrets { detail, .. } = &tab.data
        && let Some(d) = detail
        && let Some(client) = &clients.secrets
    {
        let tx = app.event_tx.clone();
        let c = client.clone();
        let secret_id = d.arn.clone();
        tokio::spawn(async move {
            let result = c.get_secret_value(&secret_id).await;
            let _ = tx
                .send(AppEvent::TabEvent(
                    tab_id,
                    TabEvent::SecretValueLoaded(result),
                ))
                .await;
        });
    }
}

pub(crate) fn load_ecs_tasks(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    if let awsui::tab::ServiceData::Ecs {
        clusters,
        services,
        nav_level,
        ..
    } = &tab.data
        && let Some(svc_idx) = nav_level.as_ref().and_then(|nl| nl.service_index())
        && let Some(service) = services.get(svc_idx)
        && let Some(cluster) = clusters.filtered.get(tab.selected_index)
        && let Some(client) = &clients.ecs
    {
        let tx = app.event_tx.clone();
        let c = client.clone();
        let cluster_arn = cluster.cluster_arn.clone();
        let service_name = service.service_name.clone();
        tokio::spawn(async move {
            let result = c.list_tasks(&cluster_arn, &service_name).await;
            let _ = tx
                .send(AppEvent::TabEvent(tab_id, TabEvent::EcsTasksLoaded(result)))
                .await;
        });
    }
}

pub(crate) fn load_s3_objects(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    if let awsui::tab::ServiceData::S3 {
        selected_bucket,
        current_prefix,
        ..
    } = &tab.data
        && let Some(client) = &clients.s3
    {
        spawn_list_objects(
            &app.event_tx,
            client,
            selected_bucket,
            current_prefix,
            tab_id,
        );
    }
}

pub(crate) fn refresh_list_data(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    let tx = app.event_tx.clone();

    match tab.service {
        awsui::service::ServiceKind::Ec2 => {
            if let Some(client) = &clients.ec2 {
                load_instances(&tx, client.clone(), tab_id);
            }
        }
        awsui::service::ServiceKind::Ecr => {
            if let Some(client) = &clients.ecr {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.describe_repositories().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(
                            tab_id,
                            TabEvent::RepositoriesLoaded(result),
                        ))
                        .await;
                });
            }
        }
        awsui::service::ServiceKind::Ecs => {
            if let Some(client) = &clients.ecs {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_clusters().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::ClustersLoaded(result)))
                        .await;
                });
            }
        }
        awsui::service::ServiceKind::S3 => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_buckets().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::BucketsLoaded(result)))
                        .await;
                });
            }
        }
        awsui::service::ServiceKind::Vpc => {
            if let Some(client) = &clients.vpc {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.describe_vpcs().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::VpcsLoaded(result)))
                        .await;
                });
            }
        }
        awsui::service::ServiceKind::SecretsManager => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_secrets().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::SecretsLoaded(result)))
                        .await;
                });
            }
        }
    }
}

/// ECSタスク定義のログ設定を取得する
pub(crate) fn load_ecs_log_configs(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    if let awsui::tab::ServiceData::Ecs {
        tasks, nav_level, ..
    } = &tab.data
        && let Some(task_idx) = nav_level.as_ref().and_then(|nl| nl.task_index())
        && let Some(task) = tasks.get(task_idx)
        && let Some(client) = &clients.ecs
    {
        let tx = app.event_tx.clone();
        let c = client.clone();
        let task_def_arn = task.task_definition_arn.clone();
        tokio::spawn(async move {
            let result = c.describe_task_definition_log_configs(&task_def_arn).await;
            let _ = tx
                .send(AppEvent::TabEvent(
                    tab_id,
                    TabEvent::EcsLogConfigsLoaded(result),
                ))
                .await;
        });
    }
}

/// CRUD完了後にリストをリフレッシュする
pub(crate) fn trigger_refresh(app: &App, clients: &Clients) {
    let Some(tab) = app.active_tab() else {
        return;
    };
    let tab_id = tab.id;
    let tx = app.event_tx.clone();

    match app.current_view() {
        Some((ServiceKind::Ec2, TabView::List)) => {
            if let Some(client) = &clients.ec2 {
                load_instances(&tx, client.clone(), tab_id);
            }
        }
        Some((ServiceKind::S3, TabView::List)) => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_buckets().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::BucketsLoaded(result)))
                        .await;
                });
            }
        }
        Some((ServiceKind::S3, TabView::Detail)) => {
            if let awsui::tab::ServiceData::S3 {
                selected_bucket,
                current_prefix,
                ..
            } = &tab.data
                && let Some(client) = &clients.s3
            {
                spawn_list_objects(&tx, client, selected_bucket, current_prefix, tab_id);
            }
        }
        Some((ServiceKind::SecretsManager, TabView::List)) => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_secrets().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::SecretsLoaded(result)))
                        .await;
                });
            }
        }
        Some((ServiceKind::SecretsManager, TabView::Detail)) => {
            if let awsui::tab::ServiceData::Secrets { detail, .. } = &tab.data
                && let Some(detail) = detail
                && let Some(client) = &clients.secrets
            {
                let c = client.clone();
                let secret_id = detail.arn.clone();
                tokio::spawn(async move {
                    let result = c.describe_secret(&secret_id).await;
                    let _ = tx
                        .send(AppEvent::TabEvent(
                            tab_id,
                            TabEvent::SecretDetailLoaded(result.map(Box::new)),
                        ))
                        .await;
                });
            }
        }
        _ => {}
    }
}
