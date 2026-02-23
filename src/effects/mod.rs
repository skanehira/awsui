mod crud;
mod log;
mod terminal;

use std::sync::Arc;

use awsui::app::{App, ConfirmAction, SideEffect};
use awsui::aws::client::Ec2Client;
use awsui::aws::ecr_client::EcrClient;
use awsui::aws::ecs_client::EcsClient;
use awsui::aws::s3_client::S3Client;
use awsui::aws::secrets_client::SecretsClient;
use awsui::aws::vpc_client::VpcClient;
#[cfg(not(feature = "mock-data"))]
use awsui::aws::{
    client::AwsEc2Client, ecr_client::AwsEcrClient, ecs_client::AwsEcsClient,
    s3_client::AwsS3Client, secrets_client::AwsSecretsClient, vpc_client::AwsVpcClient,
};
use awsui::event::{AppEvent, TabEvent};
use awsui::service::ServiceKind;
use awsui::tab::{TabId, TabView};

use crate::clients::Clients;
use crate::loader::{
    load_detail_data, load_ecs_log_configs, load_ecs_tasks, load_instances, load_s3_objects,
    load_secret_value, refresh_list_data,
};

// サブモジュールの関数を再公開
pub(crate) use log::{fetch_log_events, manage_log_polling};
pub(crate) use terminal::{cancel_sso_login, run_ecs_exec, run_ssm_connect, start_sso_login};

/// handle_side_effects でデータ読み込み条件を明示化する列挙型。
/// prev_view / current_view / tab_loading / action の組み合わせ条件を
/// 名前付きバリアントとして表現し、条件判定と実行を分離する。
#[derive(Debug, PartialEq)]
enum DataLoadAction {
    /// ShowLogs → タスク定義のログ設定取得
    LoadLogConfigs(TabId),
    /// LogView && loading → ログイベント初回取得
    FetchLogEvents {
        tab_id: TabId,
        log_group: String,
        log_stream: String,
        next_token: Option<String>,
    },
    /// 新規タブ → クライアント作成 + データ読み込み
    CreateClientAndLoad(TabId),
    /// List → Detail 遷移 → 詳細データ読み込み
    LoadDetailData(TabId),
    /// ECS サービス → タスク一覧読み込み
    LoadEcsTasks(TabId),
    /// S3 プレフィックスナビゲーション
    LoadS3Objects(TabId),
    /// EC2 → VPC ナビゲーションリンク
    NavigateVpcLink(TabId),
    /// シークレット値取得
    LoadSecretValue(TabId),
    /// リスト画面でのリフレッシュ
    RefreshListData(TabId),
    /// データ読み込み不要
    None,
}

fn determine_data_load_action(
    app: &App,
    side_effect: &SideEffect,
    prev_view: &Option<(ServiceKind, TabView)>,
    prev_tab_id: Option<TabId>,
    action: &awsui::action::Action,
) -> DataLoadAction {
    let Some(tab) = app.active_tab() else {
        return DataLoadAction::None;
    };
    let tab_id = tab.id;
    let tab_loading = tab.loading;
    let tab_view = tab.tab_view;

    // ShowLogs → タスク定義のログ設定取得
    if matches!(action, awsui::action::Action::ShowLogs) && tab_loading {
        return DataLoadAction::LoadLogConfigs(tab_id);
    }

    // ログ初回取得（log_stateがありloading=trueの場合、ContainerSelectConfirm後など）
    if tab_loading
        && let awsui::tab::ServiceData::Ecs {
            nav_level: Some(awsui::tab::EcsNavLevel::LogView { log_state, .. }),
            ..
        } = &tab.data
    {
        return DataLoadAction::FetchLogEvents {
            tab_id,
            log_group: log_state.log_group.clone(),
            log_stream: log_state.log_stream.clone(),
            next_token: log_state.next_forward_token.clone(),
        };
    }

    // 新しいタブが作成された場合
    if tab_loading
        && tab_view == awsui::tab::TabView::List
        && (prev_view.is_none() || prev_tab_id != Some(tab_id))
    {
        return DataLoadAction::CreateClientAndLoad(tab_id);
    }

    // ディテールビューに遷移した場合
    if tab_loading && tab_view == awsui::tab::TabView::Detail {
        let was_list = prev_view.as_ref().is_some_and(is_list_view);
        let was_same_detail = prev_view.as_ref().is_some_and(is_detail_view);

        if was_list {
            return DataLoadAction::LoadDetailData(tab_id);
        }

        if was_same_detail {
            let current_view = app.current_view();
            return match current_view {
                Some((ServiceKind::Ecs, TabView::Detail)) => DataLoadAction::LoadEcsTasks(tab_id),
                Some((ServiceKind::S3, TabView::Detail)) => DataLoadAction::LoadS3Objects(tab_id),
                Some((ServiceKind::Vpc, TabView::Detail)) => {
                    DataLoadAction::NavigateVpcLink(tab_id)
                }
                Some((ServiceKind::SecretsManager, TabView::Detail)) => {
                    DataLoadAction::LoadSecretValue(tab_id)
                }
                _ => DataLoadAction::None,
            };
        }
    }

    // リフレッシュ（既にリストビューにいて loading が true の場合）
    let current_view = app.current_view();
    if tab_loading
        && matches!(side_effect, SideEffect::None)
        && tab_view == awsui::tab::TabView::List
        && prev_view
            .as_ref()
            .is_some_and(|v| current_view.as_ref() == Some(v))
    {
        return DataLoadAction::RefreshListData(tab_id);
    }

    DataLoadAction::None
}

pub(crate) async fn handle_side_effects(
    app: &mut App,
    clients: &mut Clients,
    side_effect: SideEffect,
    prev_view: &Option<(ServiceKind, TabView)>,
    prev_tab_id: Option<TabId>,
    action_clone: &awsui::action::Action,
) {
    // ダッシュボードに戻った場合はクライアントをクリア
    if app.show_dashboard && prev_view.is_some() {
        clients.clear();
    }

    // データ読み込みアクションを判定
    let data_action =
        determine_data_load_action(app, &side_effect, prev_view, prev_tab_id, action_clone);

    // データ読み込みアクションを実行
    match data_action {
        DataLoadAction::LoadLogConfigs(tab_id) => {
            load_ecs_log_configs(app, clients, tab_id);
            return;
        }
        DataLoadAction::FetchLogEvents {
            tab_id,
            log_group,
            log_stream,
            next_token,
        } => {
            fetch_log_events(app, clients, tab_id, &log_group, &log_stream, next_token).await;
            return;
        }
        DataLoadAction::CreateClientAndLoad(tab_id) => {
            create_client_and_load(app, clients, tab_id).await;
            return;
        }
        DataLoadAction::LoadDetailData(tab_id) => {
            load_detail_data(app, clients, tab_id);
            return;
        }
        DataLoadAction::LoadEcsTasks(tab_id) => {
            load_ecs_tasks(app, clients, tab_id);
            return;
        }
        DataLoadAction::LoadS3Objects(tab_id) => {
            load_s3_objects(app, clients, tab_id);
            return;
        }
        DataLoadAction::NavigateVpcLink(tab_id) => {
            handle_navigation_link(app, clients, tab_id).await;
            return;
        }
        DataLoadAction::LoadSecretValue(tab_id) => {
            load_secret_value(app, clients, tab_id);
            return;
        }
        DataLoadAction::RefreshListData(tab_id) => {
            refresh_list_data(app, clients, tab_id);
            return;
        }
        DataLoadAction::None => {}
    }

    // 明示的な副作用の処理
    let Some(tab_id) = app.active_tab().map(|t| t.id) else {
        return;
    };

    match side_effect {
        SideEffect::Confirm(action) => {
            if let Some(client) = clients.ec2.clone() {
                let tx = app.event_tx.clone();
                match action {
                    ConfirmAction::Start(id) => {
                        tokio::spawn(async move {
                            let result = client.start_instances(std::slice::from_ref(&id)).await;
                            let event = match result {
                                Ok(()) => AppEvent::TabEvent(
                                    tab_id,
                                    TabEvent::ActionCompleted(Ok(format!(
                                        "Instance {} started",
                                        id
                                    ))),
                                ),
                                Err(e) => {
                                    AppEvent::TabEvent(tab_id, TabEvent::ActionCompleted(Err(e)))
                                }
                            };
                            let _ = tx.send(event).await;
                        });
                    }
                    ConfirmAction::Stop(id) => {
                        tokio::spawn(async move {
                            let result = client.stop_instances(std::slice::from_ref(&id)).await;
                            let event = match result {
                                Ok(()) => AppEvent::TabEvent(
                                    tab_id,
                                    TabEvent::ActionCompleted(Ok(format!(
                                        "Instance {} stopped",
                                        id
                                    ))),
                                ),
                                Err(e) => {
                                    AppEvent::TabEvent(tab_id, TabEvent::ActionCompleted(Err(e)))
                                }
                            };
                            let _ = tx.send(event).await;
                        });
                    }
                    ConfirmAction::Reboot(id) => {
                        tokio::spawn(async move {
                            let result = client.reboot_instances(std::slice::from_ref(&id)).await;
                            let event = match result {
                                Ok(()) => AppEvent::TabEvent(
                                    tab_id,
                                    TabEvent::ActionCompleted(Ok(format!(
                                        "Instance {} rebooted",
                                        id
                                    ))),
                                ),
                                Err(e) => {
                                    AppEvent::TabEvent(tab_id, TabEvent::ActionCompleted(Err(e)))
                                }
                            };
                            let _ = tx.send(event).await;
                        });
                    }
                }
            }
        }
        SideEffect::FormSubmit(form_ctx) => {
            crud::handle_form_side_effect(app, clients, form_ctx, tab_id);
        }
        SideEffect::DangerAction(danger_action) => {
            crud::handle_danger_side_effect(app, clients, danger_action, tab_id);
        }
        SideEffect::StartSsoLogin { .. } => {
            // メインループで直接処理済み
        }
        SideEffect::SsmConnect { .. } => {
            // メインループで直接処理済み
        }
        SideEffect::EcsExec { .. } => {
            // メインループで直接処理済み
        }
        SideEffect::None => {}
    }
}

#[cfg(feature = "mock-data")]
async fn create_client_and_load(app: &mut App, clients: &mut Clients, tab_id: TabId) {
    use awsui::aws::mock_clients::*;

    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    let service = tab.service;

    match service {
        awsui::service::ServiceKind::Ec2 => {
            let client: Arc<dyn Ec2Client> = Arc::new(MockEc2ClientImpl);
            load_instances(&app.event_tx, client.clone(), tab_id);
            clients.ec2 = Some(client);
        }
        awsui::service::ServiceKind::Ecr => {
            let client: Arc<dyn EcrClient> = Arc::new(MockEcrClientImpl);
            let tx = app.event_tx.clone();
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
            clients.ecr = Some(client);
        }
        awsui::service::ServiceKind::Ecs => {
            let client: Arc<dyn EcsClient> = Arc::new(MockEcsClientImpl);
            let tx = app.event_tx.clone();
            let c = client.clone();
            tokio::spawn(async move {
                let result = c.list_clusters().await;
                let _ = tx
                    .send(AppEvent::TabEvent(tab_id, TabEvent::ClustersLoaded(result)))
                    .await;
            });
            clients.ecs = Some(client);
        }
        awsui::service::ServiceKind::S3 => {
            let client: Arc<dyn S3Client> = Arc::new(MockS3ClientImpl);
            let tx = app.event_tx.clone();
            let c = client.clone();
            tokio::spawn(async move {
                let result = c.list_buckets().await;
                let _ = tx
                    .send(AppEvent::TabEvent(tab_id, TabEvent::BucketsLoaded(result)))
                    .await;
            });
            clients.s3 = Some(client);
        }
        awsui::service::ServiceKind::Vpc => {
            let client: Arc<dyn VpcClient> = Arc::new(MockVpcClientImpl);
            let tx = app.event_tx.clone();
            let c = client.clone();
            tokio::spawn(async move {
                let result = c.describe_vpcs().await;
                let _ = tx
                    .send(AppEvent::TabEvent(tab_id, TabEvent::VpcsLoaded(result)))
                    .await;
            });
            clients.vpc = Some(client);
        }
        awsui::service::ServiceKind::SecretsManager => {
            let client: Arc<dyn SecretsClient> = Arc::new(MockSecretsClientImpl);
            let tx = app.event_tx.clone();
            let c = client.clone();
            tokio::spawn(async move {
                let result = c.list_secrets().await;
                let _ = tx
                    .send(AppEvent::TabEvent(tab_id, TabEvent::SecretsLoaded(result)))
                    .await;
            });
            clients.secrets = Some(client);
        }
    }
}

/// クライアント生成エラー時の共通処理
fn handle_client_error(app: &mut App, tab_id: TabId, e: impl std::fmt::Display) {
    if let Some(t) = app.find_tab_mut(tab_id) {
        t.loading = false;
    }
    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
}

#[cfg(not(feature = "mock-data"))]
async fn create_client_and_load(app: &mut App, clients: &mut Clients, tab_id: TabId) {
    let Some(profile_name) = &app.profile else {
        return;
    };
    let region = app
        .region
        .clone()
        .unwrap_or_else(|| "ap-northeast-1".to_string());

    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    let service = tab.service;

    match service {
        awsui::service::ServiceKind::Ec2 => match AwsEc2Client::new(profile_name, &region).await {
            Ok(client) => {
                let client: Arc<dyn Ec2Client> = Arc::new(client);
                load_instances(&app.event_tx, client.clone(), tab_id);
                clients.ec2 = Some(client);
            }
            Err(e) => handle_client_error(app, tab_id, e),
        },
        awsui::service::ServiceKind::Ecr => match AwsEcrClient::new(profile_name, &region).await {
            Ok(client) => {
                let client: Arc<dyn EcrClient> = Arc::new(client);
                let tx = app.event_tx.clone();
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
                clients.ecr = Some(client);
            }
            Err(e) => handle_client_error(app, tab_id, e),
        },
        awsui::service::ServiceKind::Ecs => match AwsEcsClient::new(profile_name, &region).await {
            Ok(client) => {
                let client: Arc<dyn EcsClient> = Arc::new(client);
                let tx = app.event_tx.clone();
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_clusters().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::ClustersLoaded(result)))
                        .await;
                });
                clients.ecs = Some(client);
            }
            Err(e) => handle_client_error(app, tab_id, e),
        },
        awsui::service::ServiceKind::S3 => match AwsS3Client::new(profile_name, &region).await {
            Ok(client) => {
                let client: Arc<dyn S3Client> = Arc::new(client);
                let tx = app.event_tx.clone();
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_buckets().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::BucketsLoaded(result)))
                        .await;
                });
                clients.s3 = Some(client);
            }
            Err(e) => handle_client_error(app, tab_id, e),
        },
        awsui::service::ServiceKind::Vpc => match AwsVpcClient::new(profile_name, &region).await {
            Ok(client) => {
                let client: Arc<dyn VpcClient> = Arc::new(client);
                let tx = app.event_tx.clone();
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.describe_vpcs().await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::VpcsLoaded(result)))
                        .await;
                });
                clients.vpc = Some(client);
            }
            Err(e) => handle_client_error(app, tab_id, e),
        },
        awsui::service::ServiceKind::SecretsManager => {
            match AwsSecretsClient::new(profile_name, &region).await {
                Ok(client) => {
                    let client: Arc<dyn SecretsClient> = Arc::new(client);
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.list_secrets().await;
                        let _ = tx
                            .send(AppEvent::TabEvent(tab_id, TabEvent::SecretsLoaded(result)))
                            .await;
                    });
                    clients.secrets = Some(client);
                }
                Err(e) => handle_client_error(app, tab_id, e),
            }
        }
    }
}

async fn handle_navigation_link(app: &mut App, clients: &mut Clients, tab_id: TabId) {
    let navigate_target_id = app
        .find_tab_mut(tab_id)
        .and_then(|t| t.navigate_target_id.take());

    let Some(target_id) = navigate_target_id else {
        return;
    };

    // VPCクライアントがない場合は作成
    if clients.vpc.is_none() {
        #[cfg(feature = "mock-data")]
        {
            clients.vpc = Some(Arc::new(awsui::aws::mock_clients::MockVpcClientImpl));
        }
        #[cfg(not(feature = "mock-data"))]
        {
            let Some(profile_name) = &app.profile else {
                return;
            };
            let region = app
                .region
                .clone()
                .unwrap_or_else(|| "ap-northeast-1".to_string());
            match AwsVpcClient::new(profile_name, &region).await {
                Ok(client) => {
                    clients.vpc = Some(Arc::new(client));
                }
                Err(e) => {
                    // クライアント作成失敗時はスタックを巻き戻す
                    if let Some(tab) = app.find_tab_mut(tab_id) {
                        if let Some(entry) = tab.navigation_stack.pop() {
                            tab.tab_view = entry.view.1;
                            tab.selected_index = entry.selected_index;
                            tab.detail_tag_index = entry.detail_tag_index;
                            tab.detail_tab = entry.detail_tab;
                        }
                        tab.loading = false;
                    }
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                    return;
                }
            }
        }
    }
    if let Some(client) = &clients.vpc {
        let tx = app.event_tx.clone();
        let c = client.clone();
        let tid = target_id.clone();
        tokio::spawn(async move {
            // VPCリストを取得し、ターゲットのVPC IDまたはSubnet IDに該当するVPCを特定
            let vpcs_result = c.describe_vpcs().await;
            match vpcs_result {
                Ok(vpcs) => {
                    // target_idがsubnet-で始まる場合はsubnet_idから該当VPCを探す
                    let vpc_id = if tid.starts_with("subnet-") {
                        // まず全VPCのサブネットを取得して、該当するものを探す
                        let mut found_vpc_id = None;
                        for vpc in &vpcs {
                            if let Ok(subnets) = c.describe_subnets(&vpc.vpc_id).await
                                && subnets.iter().any(|s| s.subnet_id == tid)
                            {
                                found_vpc_id = Some(vpc.vpc_id.clone());
                                break;
                            }
                        }
                        found_vpc_id.unwrap_or(tid)
                    } else {
                        tid
                    };
                    // 該当VPCのサブネットを取得
                    let subnets_result = c.describe_subnets(&vpc_id).await;
                    match subnets_result {
                        Ok(subnets) => {
                            let _ = tx
                                .send(AppEvent::TabEvent(
                                    tab_id,
                                    TabEvent::NavigateVpcLoaded(Ok((vpcs, subnets))),
                                ))
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(AppEvent::TabEvent(
                                    tab_id,
                                    TabEvent::NavigateVpcLoaded(Err(e)),
                                ))
                                .await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(AppEvent::TabEvent(
                            tab_id,
                            TabEvent::NavigateVpcLoaded(Err(e)),
                        ))
                        .await;
                }
            }
        });
    }
}

fn is_list_view(view: &(ServiceKind, TabView)) -> bool {
    view.1 == TabView::List
}

fn is_detail_view(view: &(ServiceKind, TabView)) -> bool {
    view.1 == TabView::Detail
}

#[cfg(test)]
mod tests {
    use super::*;
    use awsui::action::Action;
    use awsui::app::SideEffect;
    use awsui::service::ServiceKind;
    use awsui::tab::TabView;

    #[test]
    fn determine_returns_none_when_no_active_tab() {
        let app = App::new("dev".to_string(), None);
        // show_dashboard = true, no tabs
        let result =
            determine_data_load_action(&app, &SideEffect::None, &None, None, &Action::Noop);
        assert_eq!(result, DataLoadAction::None);
    }

    #[test]
    fn determine_returns_load_log_configs_when_show_logs_and_loading() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ecs);
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::Ecs, TabView::Detail)),
            app.active_tab().map(|t| t.id),
            &Action::ShowLogs,
        );
        assert!(matches!(result, DataLoadAction::LoadLogConfigs(_)));
    }

    #[test]
    fn determine_returns_create_client_when_new_tab_loading() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        let tab_id = app.active_tab().unwrap().id;
        // new tab: loading=true, tab_view=List, prev_tab_id differs
        if let Some(tab) = app.active_tab_mut() {
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &None, // prev_view is None (was dashboard)
            None,  // prev_tab_id is None
            &Action::Enter,
        );
        assert_eq!(result, DataLoadAction::CreateClientAndLoad(tab_id));
    }

    #[test]
    fn determine_returns_load_detail_data_when_list_to_detail() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::Ec2, TabView::List)), // was List
            Some(tab_id),
            &Action::Enter,
        );
        assert_eq!(result, DataLoadAction::LoadDetailData(tab_id));
    }

    #[test]
    fn determine_returns_load_ecs_tasks_when_detail_to_detail_ecs() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ecs);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::Ecs, TabView::Detail)), // was Detail
            Some(tab_id),
            &Action::Enter,
        );
        assert_eq!(result, DataLoadAction::LoadEcsTasks(tab_id));
    }

    #[test]
    fn determine_returns_load_s3_objects_when_detail_to_detail_s3() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::S3);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::S3, TabView::Detail)),
            Some(tab_id),
            &Action::Enter,
        );
        assert_eq!(result, DataLoadAction::LoadS3Objects(tab_id));
    }

    #[test]
    fn determine_returns_navigate_vpc_link_when_detail_to_detail_vpc() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Vpc);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::Vpc, TabView::Detail)),
            Some(tab_id),
            &Action::Enter,
        );
        assert_eq!(result, DataLoadAction::NavigateVpcLink(tab_id));
    }

    #[test]
    fn determine_returns_load_secret_value_when_detail_to_detail_secrets() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::SecretsManager);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::SecretsManager, TabView::Detail)),
            Some(tab_id),
            &Action::Enter,
        );
        assert_eq!(result, DataLoadAction::LoadSecretValue(tab_id));
    }

    #[test]
    fn determine_returns_refresh_list_data_when_same_list_view_loading() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.loading = true;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::Ec2, TabView::List)), // same view
            Some(tab_id),
            &Action::Refresh,
        );
        assert_eq!(result, DataLoadAction::RefreshListData(tab_id));
    }

    #[test]
    fn determine_returns_none_when_not_loading() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.loading = false;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::Ec2, TabView::List)),
            Some(tab_id),
            &Action::MoveDown,
        );
        assert_eq!(result, DataLoadAction::None);
    }

    #[test]
    fn determine_returns_none_when_detail_not_loading() {
        let mut app = App::new("dev".to_string(), None);
        app.create_tab(ServiceKind::Ec2);
        let tab_id = app.active_tab().unwrap().id;
        if let Some(tab) = app.active_tab_mut() {
            tab.tab_view = TabView::Detail;
            tab.loading = false;
        }
        let result = determine_data_load_action(
            &app,
            &SideEffect::None,
            &Some((ServiceKind::Ec2, TabView::List)),
            Some(tab_id),
            &Action::Enter,
        );
        assert_eq!(result, DataLoadAction::None);
    }
}
