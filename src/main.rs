use std::sync::Arc;

use awsui::app::{App, ConfirmAction, Mode, View};
use awsui::aws::client::{AwsEc2Client, Ec2Client};
use awsui::aws::ecr_client::{AwsEcrClient, EcrClient};
use awsui::aws::ecs_client::{AwsEcsClient, EcsClient};
use awsui::aws::s3_client::{AwsS3Client, S3Client};
use awsui::aws::secrets_client::{AwsSecretsClient, SecretsClient};
use awsui::aws::vpc_client::{AwsVpcClient, VpcClient};
use awsui::cli::{Cli, DeletePermissions};
use awsui::config;
use awsui::event::AppEvent;
use awsui::tui::components::dialog::{ConfirmDialog, MessageDialog};
use awsui::tui::components::form_dialog::{DangerConfirmDialog, FormDialog};
use awsui::tui::components::help::HelpPopup;
use clap::Parser;
use ratatui::Frame;
use tokio::sync::mpsc;

/// 各サービスのクライアントをまとめて保持する
struct Clients {
    ec2: Option<Arc<dyn Ec2Client>>,
    ecr: Option<Arc<dyn EcrClient>>,
    ecs: Option<Arc<dyn EcsClient>>,
    s3: Option<Arc<dyn S3Client>>,
    vpc: Option<Arc<dyn VpcClient>>,
    secrets: Option<Arc<dyn SecretsClient>>,
}

impl Clients {
    fn new() -> Self {
        Self {
            ec2: None,
            ecr: None,
            ecs: None,
            s3: None,
            vpc: None,
            secrets: None,
        }
    }

    fn clear(&mut self) {
        self.ec2 = None;
        self.ecr = None;
        self.ecs = None;
        self.s3 = None;
        self.vpc = None;
        self.secrets = None;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. CLI引数パース
    let cli = Cli::parse();
    let delete_permissions =
        DeletePermissions::from_cli(cli.allow_delete.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

    // 1. SSOプロファイル読み込み
    let profiles = config::load_sso_profiles()?;
    let profile_names = config::profile_names(&profiles);

    if profile_names.is_empty() {
        eprintln!("No SSO profiles found in ~/.aws/config");
        return Ok(());
    }

    // 2. App初期化
    let mut app = App::with_delete_permissions(profile_names.clone(), delete_permissions);

    // 3. ターミナル初期化
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 4. メインループ
    let mut event_stream = crossterm::event::EventStream::new();
    let mut render_interval = tokio::time::interval(std::time::Duration::from_millis(16));
    let mut clients = Clients::new();
    let mut spinner_tick: usize = 0;

    loop {
        tokio::select! {
            Some(event) = app.event_rx.recv() => {
                let needs_ec2_refresh = matches!(&event, AppEvent::ActionCompleted(Ok(_)));
                let needs_crud_refresh = matches!(&event, AppEvent::CrudCompleted(Ok(_)));
                app.handle_event(event);
                if needs_ec2_refresh
                    && let Some(client) = &clients.ec2 {
                    load_instances(&app.event_tx, client.clone());
                }
                if needs_crud_refresh {
                    trigger_refresh(&app, &clients);
                }
            }
            maybe_event = futures::StreamExt::next(&mut event_stream) => {
                if let Some(Ok(crossterm::event::Event::Key(key))) = maybe_event
                    && key.kind == crossterm::event::KeyEventKind::Press
                {
                    let prev_view = app.view.clone();
                    let action = awsui::tui::input::handle_key(&app, key);
                    let confirmed = app.dispatch(action);

                    handle_side_effects(&mut app, &profiles, &mut clients, confirmed, &prev_view).await;
                }
            }
            _ = render_interval.tick() => {
                if app.loading {
                    spinner_tick = spinner_tick.wrapping_add(1);
                }
                terminal.draw(|frame| render(frame, &app, spinner_tick))?;
            }
        }

        if app.should_quit {
            break;
        }
    }

    // 5. ターミナル復元
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn render(frame: &mut Frame, app: &App, spinner_tick: usize) {
    match app.view {
        View::ProfileSelect => awsui::tui::views::profile_select::render(frame, app),
        View::ServiceSelect => {
            awsui::tui::views::service_select::render(frame, app.service_selected)
        }
        View::Ec2List => awsui::tui::views::ec2_list::render(frame, app, spinner_tick),
        View::Ec2Detail => awsui::tui::views::ec2_detail::render(frame, app),
        View::EcrList => awsui::tui::views::ecr_list::render(
            frame,
            &app.ecr_filtered_repositories,
            app.selected_index,
            &app.filter_input,
            &app.mode,
            app.loading,
            spinner_tick,
            app.profile.as_deref(),
            app.region.as_deref(),
        ),
        View::EcrDetail => {
            if let Some(repo) = app.ecr_filtered_repositories.get(app.selected_index) {
                awsui::tui::views::ecr_detail::render(
                    frame,
                    repo,
                    &app.ecr_images,
                    app.detail_tag_index,
                    app.loading,
                    spinner_tick,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                );
            }
        }
        View::EcsList => awsui::tui::views::ecs_list::render(
            frame,
            &app.ecs_filtered_clusters,
            app.selected_index,
            &app.filter_input,
            &app.mode,
            app.loading,
        ),
        View::EcsDetail => {
            if let Some(cluster) = app.ecs_filtered_clusters.get(app.selected_index) {
                awsui::tui::views::ecs_detail::render(
                    frame,
                    cluster,
                    &app.ecs_services,
                    app.detail_tag_index,
                    app.loading,
                );
            }
        }
        View::S3List => awsui::tui::views::s3_list::render(
            frame,
            &app.s3_filtered_buckets,
            app.selected_index,
            &app.filter_input,
            &app.mode,
            app.loading,
            app.profile.as_deref(),
            app.region.as_deref(),
            spinner_tick,
        ),
        View::S3Detail => {
            if let Some(bucket_name) = &app.s3_selected_bucket {
                awsui::tui::views::s3_detail::render(
                    frame,
                    bucket_name,
                    &app.s3_objects,
                    &app.s3_current_prefix,
                    app.detail_tag_index,
                    app.loading,
                    spinner_tick,
                );
            }
        }
        View::VpcList => awsui::tui::views::vpc_list::render(
            frame,
            &app.filtered_vpcs,
            app.selected_index,
            &app.filter_input,
            &app.mode,
            app.loading,
            spinner_tick,
            app.profile.as_deref(),
            app.region.as_deref(),
        ),
        View::VpcDetail => {
            if let Some(vpc) = app.filtered_vpcs.get(app.selected_index) {
                awsui::tui::views::vpc_detail::render(
                    frame,
                    vpc,
                    &app.subnets,
                    app.detail_tag_index,
                    app.loading,
                    spinner_tick,
                );
            }
        }
        View::SecretsList => awsui::tui::views::secrets_list::render(
            frame,
            &app.filtered_secrets,
            app.selected_index,
            &app.filter_input,
            &app.mode,
            app.loading,
            app.profile.as_deref(),
            app.region.as_deref(),
            spinner_tick,
        ),
        View::SecretsDetail => {
            if let Some(detail) = &app.secret_detail {
                awsui::tui::views::secrets_detail::render(
                    frame,
                    detail,
                    app.detail_tag_index,
                    &app.secrets_detail_tab,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                );
            }
        }
    }

    // モーダルオーバーレイ
    match &app.mode {
        Mode::Confirm(action) => {
            let msg = match action {
                ConfirmAction::Start(id) => format!("Start instance {}?", id),
                ConfirmAction::Stop(id) => format!("Stop instance {}?", id),
                ConfirmAction::Reboot(id) => format!("Reboot instance {}?", id),
            };
            let dialog = ConfirmDialog::new(&msg);
            frame.render_widget(dialog, frame.area());
        }
        Mode::Message => {
            if let Some(msg) = &app.message {
                let dialog = MessageDialog::new(msg);
                frame.render_widget(dialog, frame.area());
            }
        }
        Mode::Help => {
            let help = HelpPopup::new();
            frame.render_widget(help, frame.area());
        }
        Mode::Form(ctx) => {
            let dialog = FormDialog::new(ctx);
            frame.render_widget(dialog, frame.area());
        }
        Mode::DangerConfirm(ctx) => {
            let dialog = DangerConfirmDialog::new(ctx);
            frame.render_widget(dialog, frame.area());
        }
        _ => {}
    }
}

fn load_instances(tx: &mpsc::Sender<AppEvent>, client: Arc<dyn Ec2Client>) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let result = client.describe_instances().await;
        let _ = tx.send(AppEvent::InstancesLoaded(result)).await;
    });
}

async fn handle_side_effects(
    app: &mut App,
    profiles: &[config::SsoProfile],
    clients: &mut Clients,
    confirmed: Option<ConfirmAction>,
    prev_view: &View,
) {
    // ServiceSelectに戻った場合はクライアントをクリア
    if app.view == View::ServiceSelect && *prev_view != View::ServiceSelect {
        clients.clear();
    }

    // ProfileSelectに戻った場合もクリア
    if app.view == View::ProfileSelect && *prev_view != View::ProfileSelect {
        clients.clear();
    }

    // リストビューに新しく遷移した場合 → クライアント作成 + データ読み込み
    if app.loading && *prev_view == View::ServiceSelect {
        let Some(profile_name) = &app.profile else {
            return;
        };
        let region = config::get_region_for_profile(profiles, profile_name)
            .unwrap_or_else(|| "ap-northeast-1".to_string());
        app.region = Some(region.clone());

        match app.view {
            View::Ec2List => match AwsEc2Client::new(profile_name, &region).await {
                Ok(client) => {
                    let client: Arc<dyn Ec2Client> = Arc::new(client);
                    load_instances(&app.event_tx, client.clone());
                    clients.ec2 = Some(client);
                }
                Err(e) => {
                    app.loading = false;
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                }
            },
            View::EcrList => match AwsEcrClient::new(profile_name, &region).await {
                Ok(client) => {
                    let client: Arc<dyn EcrClient> = Arc::new(client);
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.describe_repositories().await;
                        let _ = tx.send(AppEvent::RepositoriesLoaded(result)).await;
                    });
                    clients.ecr = Some(client);
                }
                Err(e) => {
                    app.loading = false;
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                }
            },
            View::EcsList => match AwsEcsClient::new(profile_name, &region).await {
                Ok(client) => {
                    let client: Arc<dyn EcsClient> = Arc::new(client);
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.list_clusters().await;
                        let _ = tx.send(AppEvent::ClustersLoaded(result)).await;
                    });
                    clients.ecs = Some(client);
                }
                Err(e) => {
                    app.loading = false;
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                }
            },
            View::S3List => match AwsS3Client::new(profile_name, &region).await {
                Ok(client) => {
                    let client: Arc<dyn S3Client> = Arc::new(client);
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.list_buckets().await;
                        let _ = tx.send(AppEvent::BucketsLoaded(result)).await;
                    });
                    clients.s3 = Some(client);
                }
                Err(e) => {
                    app.loading = false;
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                }
            },
            View::VpcList => match AwsVpcClient::new(profile_name, &region).await {
                Ok(client) => {
                    let client: Arc<dyn VpcClient> = Arc::new(client);
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.describe_vpcs().await;
                        let _ = tx.send(AppEvent::VpcsLoaded(result)).await;
                    });
                    clients.vpc = Some(client);
                }
                Err(e) => {
                    app.loading = false;
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                }
            },
            View::SecretsList => match AwsSecretsClient::new(profile_name, &region).await {
                Ok(client) => {
                    let client: Arc<dyn SecretsClient> = Arc::new(client);
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.list_secrets().await;
                        let _ = tx.send(AppEvent::SecretsLoaded(result)).await;
                    });
                    clients.secrets = Some(client);
                }
                Err(e) => {
                    app.loading = false;
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                }
            },
            _ => {}
        }
        return;
    }

    // ディテールビューに遷移した場合 → 詳細データ読み込み
    if app.loading && is_detail_view(&app.view) && is_list_view(prev_view) {
        match app.view {
            View::EcrDetail => {
                if let Some(repo) = app.ecr_filtered_repositories.get(app.selected_index)
                    && let Some(client) = &clients.ecr
                {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    let repo_name = repo.repository_name.clone();
                    tokio::spawn(async move {
                        let result = c.list_images(&repo_name).await;
                        let _ = tx.send(AppEvent::ImagesLoaded(result)).await;
                    });
                }
            }
            View::EcsDetail => {
                if let Some(cluster) = app.ecs_filtered_clusters.get(app.selected_index)
                    && let Some(client) = &clients.ecs
                {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    let cluster_arn = cluster.cluster_arn.clone();
                    tokio::spawn(async move {
                        let result = c.list_services(&cluster_arn).await;
                        let _ = tx.send(AppEvent::EcsServicesLoaded(result)).await;
                    });
                }
            }
            View::S3Detail => {
                if let Some(client) = &clients.s3 {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    let bucket = app.s3_selected_bucket.clone().unwrap_or_default();
                    let prefix = if app.s3_current_prefix.is_empty() {
                        None
                    } else {
                        Some(app.s3_current_prefix.clone())
                    };
                    tokio::spawn(async move {
                        let result = c.list_objects(&bucket, prefix).await;
                        let _ = tx.send(AppEvent::ObjectsLoaded(result)).await;
                    });
                }
            }
            View::VpcDetail => {
                if let Some(vpc) = app.filtered_vpcs.get(app.selected_index)
                    && let Some(client) = &clients.vpc
                {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    let vpc_id = vpc.vpc_id.clone();
                    tokio::spawn(async move {
                        let result = c.describe_subnets(&vpc_id).await;
                        let _ = tx.send(AppEvent::SubnetsLoaded(result)).await;
                    });
                }
            }
            View::SecretsDetail => {
                if let Some(secret) = app.filtered_secrets.get(app.selected_index)
                    && let Some(client) = &clients.secrets
                {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    let secret_id = secret.arn.clone();
                    tokio::spawn(async move {
                        let result = c.describe_secret(&secret_id).await;
                        let _ = tx
                            .send(AppEvent::SecretDetailLoaded(result.map(Box::new)))
                            .await;
                    });
                }
            }
            _ => {}
        }
        return;
    }

    // S3プレフィックスナビゲーション（S3Detail → S3Detail）
    if app.loading && app.view == View::S3Detail && *prev_view == View::S3Detail {
        if let Some(client) = &clients.s3 {
            let tx = app.event_tx.clone();
            let c = client.clone();
            let bucket = app.s3_selected_bucket.clone().unwrap_or_default();
            let prefix = if app.s3_current_prefix.is_empty() {
                None
            } else {
                Some(app.s3_current_prefix.clone())
            };
            tokio::spawn(async move {
                let result = c.list_objects(&bucket, prefix).await;
                let _ = tx.send(AppEvent::ObjectsLoaded(result)).await;
            });
        }
        return;
    }

    // ナビゲーションリンク（EC2 Detail → VPC Detail）
    if app.loading && app.view == View::VpcDetail && *prev_view == View::Ec2Detail {
        if let Some(target_id) = app.navigate_target_id.take() {
            // VPCクライアントがない場合は作成
            if clients.vpc.is_none() {
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
                        if let Some(entry) = app.navigation_stack.pop() {
                            app.view = entry.view;
                            app.selected_index = entry.selected_index;
                            app.detail_tag_index = entry.detail_tag_index;
                            app.detail_tab = entry.detail_tab;
                        }
                        app.loading = false;
                        app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                        return;
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
                                // 簡易的にVPC一覧から全サブネットを取得
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
                                        .send(AppEvent::NavigateVpcLoaded(Ok((vpcs, subnets))))
                                        .await;
                                }
                                Err(e) => {
                                    let _ = tx.send(AppEvent::NavigateVpcLoaded(Err(e))).await;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::NavigateVpcLoaded(Err(e))).await;
                        }
                    }
                });
            }
        }
        return;
    }

    // リフレッシュ（既にリストビューにいて loading が true の場合）
    if app.loading && confirmed.is_none() && is_list_view(&app.view) && *prev_view == app.view {
        match app.view {
            View::Ec2List => {
                if let Some(client) = &clients.ec2 {
                    load_instances(&app.event_tx, client.clone());
                }
            }
            View::EcrList => {
                if let Some(client) = &clients.ecr {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.describe_repositories().await;
                        let _ = tx.send(AppEvent::RepositoriesLoaded(result)).await;
                    });
                }
            }
            View::EcsList => {
                if let Some(client) = &clients.ecs {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.list_clusters().await;
                        let _ = tx.send(AppEvent::ClustersLoaded(result)).await;
                    });
                }
            }
            View::S3List => {
                if let Some(client) = &clients.s3 {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.list_buckets().await;
                        let _ = tx.send(AppEvent::BucketsLoaded(result)).await;
                    });
                }
            }
            View::VpcList => {
                if let Some(client) = &clients.vpc {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.describe_vpcs().await;
                        let _ = tx.send(AppEvent::VpcsLoaded(result)).await;
                    });
                }
            }
            View::SecretsList => {
                if let Some(client) = &clients.secrets {
                    let tx = app.event_tx.clone();
                    let c = client.clone();
                    tokio::spawn(async move {
                        let result = c.list_secrets().await;
                        let _ = tx.send(AppEvent::SecretsLoaded(result)).await;
                    });
                }
            }
            _ => {}
        }
        return;
    }

    // ConfirmYes → API呼び出し (EC2のみ)
    if let Some(action) = confirmed
        && let Some(client) = clients.ec2.clone()
    {
        let tx = app.event_tx.clone();
        match action {
            ConfirmAction::Start(id) => {
                tokio::spawn(async move {
                    let result = client.start_instances(std::slice::from_ref(&id)).await;
                    let event = match result {
                        Ok(()) => AppEvent::ActionCompleted(Ok(format!("Instance {} started", id))),
                        Err(e) => AppEvent::ActionCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
            ConfirmAction::Stop(id) => {
                tokio::spawn(async move {
                    let result = client.stop_instances(std::slice::from_ref(&id)).await;
                    let event = match result {
                        Ok(()) => AppEvent::ActionCompleted(Ok(format!("Instance {} stopped", id))),
                        Err(e) => AppEvent::ActionCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
            ConfirmAction::Reboot(id) => {
                tokio::spawn(async move {
                    let result = client.reboot_instances(std::slice::from_ref(&id)).await;
                    let event = match result {
                        Ok(()) => {
                            AppEvent::ActionCompleted(Ok(format!("Instance {} rebooted", id)))
                        }
                        Err(e) => AppEvent::ActionCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }

    // フォーム送信 → CRUD API呼び出し
    if let Some(form_ctx) = app.pending_form.take() {
        handle_form_side_effect(app, clients, form_ctx);
    }

    // 危険操作確認 → CRUD API呼び出し
    if let Some(danger_action) = app.pending_danger_action.take() {
        handle_danger_side_effect(app, clients, danger_action);
    }
}

fn handle_form_side_effect(app: &mut App, clients: &Clients, form_ctx: awsui::app::FormContext) {
    use awsui::app::FormKind;
    let tx = app.event_tx.clone();
    let values: Vec<String> = form_ctx
        .fields
        .iter()
        .map(|f| f.input.value().to_string())
        .collect();

    match form_ctx.kind {
        FormKind::CreateS3Bucket => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                let bucket_name = values[0].clone();
                tokio::spawn(async move {
                    let result = c.create_bucket(&bucket_name).await;
                    let event = match result {
                        Ok(()) => {
                            AppEvent::CrudCompleted(Ok(format!("Bucket '{}' created", bucket_name)))
                        }
                        Err(e) => AppEvent::CrudCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::CreateSecret => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                let name = values[0].clone();
                let value = values[1].clone();
                let description = if values.len() > 2 && !values[2].is_empty() {
                    Some(values[2].clone())
                } else {
                    None
                };
                tokio::spawn(async move {
                    let result = c.create_secret(&name, &value, description).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(Ok(format!("Secret '{}' created", name))),
                        Err(e) => AppEvent::CrudCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::UpdateSecretValue => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                let secret_id = app
                    .secret_detail
                    .as_ref()
                    .map(|d| d.arn.clone())
                    .unwrap_or_default();
                let new_value = values[0].clone();
                tokio::spawn(async move {
                    let result = c.update_secret_value(&secret_id, &new_value).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(Ok("Secret value updated".to_string())),
                        Err(e) => AppEvent::CrudCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }
}

fn handle_danger_side_effect(
    app: &mut App,
    clients: &Clients,
    danger_action: awsui::app::DangerAction,
) {
    use awsui::app::DangerAction;
    let tx = app.event_tx.clone();

    match danger_action {
        DangerAction::TerminateEc2(id) => {
            if let Some(client) = &clients.ec2 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.terminate_instances(std::slice::from_ref(&id)).await;
                    let event = match result {
                        Ok(()) => {
                            AppEvent::CrudCompleted(Ok(format!("Instance {} terminated", id)))
                        }
                        Err(e) => AppEvent::CrudCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        DangerAction::DeleteS3Bucket(name) => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.delete_bucket(&name).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(Ok(format!("Bucket '{}' deleted", name))),
                        Err(e) => AppEvent::CrudCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        DangerAction::DeleteS3Object { bucket, key } => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.delete_object(&bucket, &key).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(Ok(format!("Object '{}' deleted", key))),
                        Err(e) => AppEvent::CrudCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        DangerAction::DeleteSecret(name) => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.delete_secret(&name).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(Ok(format!("Secret '{}' deleted", name))),
                        Err(e) => AppEvent::CrudCompleted(Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }
}

/// CRUD完了後にリストをリフレッシュする
fn trigger_refresh(app: &App, clients: &Clients) {
    let tx = app.event_tx.clone();
    match app.view {
        View::Ec2List => {
            if let Some(client) = &clients.ec2 {
                load_instances(&tx, client.clone());
            }
        }
        View::S3List => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_buckets().await;
                    let _ = tx.send(AppEvent::BucketsLoaded(result)).await;
                });
            }
        }
        View::S3Detail => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                let bucket = app.s3_selected_bucket.clone().unwrap_or_default();
                let prefix = if app.s3_current_prefix.is_empty() {
                    None
                } else {
                    Some(app.s3_current_prefix.clone())
                };
                tokio::spawn(async move {
                    let result = c.list_objects(&bucket, prefix).await;
                    let _ = tx.send(AppEvent::ObjectsLoaded(result)).await;
                });
            }
        }
        View::SecretsList => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.list_secrets().await;
                    let _ = tx.send(AppEvent::SecretsLoaded(result)).await;
                });
            }
        }
        View::SecretsDetail => {
            if let Some(client) = &clients.secrets
                && let Some(detail) = &app.secret_detail
            {
                let c = client.clone();
                let secret_id = detail.arn.clone();
                tokio::spawn(async move {
                    let result = c.describe_secret(&secret_id).await;
                    let _ = tx
                        .send(AppEvent::SecretDetailLoaded(result.map(Box::new)))
                        .await;
                });
            }
        }
        _ => {}
    }
}

fn is_list_view(view: &View) -> bool {
    matches!(
        view,
        View::Ec2List
            | View::EcrList
            | View::EcsList
            | View::S3List
            | View::VpcList
            | View::SecretsList
    )
}

fn is_detail_view(view: &View) -> bool {
    matches!(
        view,
        View::Ec2Detail
            | View::EcrDetail
            | View::EcsDetail
            | View::S3Detail
            | View::VpcDetail
            | View::SecretsDetail
    )
}
