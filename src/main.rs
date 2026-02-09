use std::io::Cursor;
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
use awsui::event::{AppEvent, TabEvent};
use awsui::sso::{self, SsoTokenStatus};
use awsui::tab::TabId;
use awsui::tui::components::dialog::{ConfirmDialog, MessageDialog};
use awsui::tui::components::form_dialog::{DangerConfirmDialog, FormDialog};
use awsui::tui::components::help::HelpPopup;
use awsui::tui::components::tab_bar::TabBar;
use clap::Parser;
use ratatui::Frame;
use skim::prelude::*;
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

    // 2. skim でプロファイル選択
    let Some(selected_profile) = select_profile(&profile_names) else {
        return Ok(());
    };

    // 3. SSO トークンチェック + login
    if let Some(profile) = profiles.iter().find(|p| p.name == selected_profile) {
        match sso::check_sso_token(profile) {
            SsoTokenStatus::Valid => {}
            SsoTokenStatus::Expired | SsoTokenStatus::NotFound => {
                let status = std::process::Command::new("aws")
                    .args(["sso", "login", "--profile", &selected_profile])
                    .stdin(std::process::Stdio::inherit())
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status()?;
                if !status.success() {
                    eprintln!("aws sso login failed");
                    return Ok(());
                }
            }
        }
    }

    // 4. リージョン取得 + App初期化
    let region = config::get_region_for_profile(&profiles, &selected_profile);
    let mut app = App::with_delete_permissions(selected_profile, region, delete_permissions);

    // 5. ターミナル初期化
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 6. メインループ
    let mut event_stream = crossterm::event::EventStream::new();
    let mut render_interval = tokio::time::interval(std::time::Duration::from_millis(16));
    let mut clients = Clients::new();
    let mut spinner_tick: usize = 0;

    loop {
        tokio::select! {
            Some(event) = app.event_rx.recv() => {
                let needs_ec2_refresh = matches!(&event, AppEvent::TabEvent(_, TabEvent::ActionCompleted(Ok(_))));
                let needs_crud_refresh = matches!(&event, AppEvent::CrudCompleted(_, Ok(_)));
                app.handle_event(event);
                if needs_ec2_refresh
                    && let Some(tab) = app.active_tab()
                    && let Some(client) = &clients.ec2
                {
                    load_instances(&app.event_tx, client.clone(), tab.id);
                }
                if needs_crud_refresh {
                    trigger_refresh(&app, &clients);
                }
            }
            maybe_event = futures::StreamExt::next(&mut event_stream) => {
                if let Some(Ok(crossterm::event::Event::Key(key))) = maybe_event
                    && key.kind == crossterm::event::KeyEventKind::Press
                {
                    let prev_view = app.current_view();
                    let prev_tab_id = app.active_tab().map(|t| t.id);
                    let action = awsui::tui::input::handle_key(&app, key);
                    let confirmed = app.dispatch(action);

                    handle_side_effects(&mut app, &mut clients, confirmed, &prev_view, prev_tab_id).await;
                }
            }
            _ = render_interval.tick() => {
                let is_loading = app.active_tab().map(|t| t.loading).unwrap_or(false);
                if is_loading {
                    spinner_tick = spinner_tick.wrapping_add(1);
                }
                terminal.draw(|frame| render(frame, &app, spinner_tick))?;
            }
        }

        if app.should_quit {
            break;
        }
    }

    // 7. ターミナル復元
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    Ok(())
}

/// skim を使ってプロファイルを選択する
fn select_profile(profile_names: &[String]) -> Option<String> {
    let options = SkimOptionsBuilder::default()
        .height("100%".to_string())
        .prompt("AWS Profile> ".to_string())
        .build()
        .unwrap();

    let input = profile_names.join("\n");
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input));

    let output = Skim::run_with(&options, Some(items))?;
    if output.is_abort {
        return None;
    }

    output
        .selected_items
        .first()
        .map(|item| item.output().to_string())
}

fn render(frame: &mut Frame, app: &App, spinner_tick: usize) {
    if app.show_dashboard {
        awsui::tui::views::dashboard::render(frame, app);
    } else if let Some(tab) = app.active_tab() {
        // タブが2つ以上の場合のみタブバーを表示
        if app.tabs.len() > 1 {
            let area = frame.area();
            let chunks = ratatui::layout::Layout::vertical([
                ratatui::layout::Constraint::Length(1), // タブバー
                ratatui::layout::Constraint::Min(1),    // コンテンツ
            ])
            .split(area);

            let tab_bar = TabBar::new(&app.tabs, app.active_tab_index);
            frame.render_widget(tab_bar, chunks[0]);

            // コンテンツ部分を描画（サブフレーム的にclipする）
            render_tab_content(frame, app, tab, spinner_tick, chunks[1]);
        } else {
            render_tab_content(frame, app, tab, spinner_tick, frame.area());
        }

        // モーダルオーバーレイ（タブ固有モード）
        render_tab_overlays(frame, tab);

        // グローバルオーバーレイ
        render_global_overlays(frame, app);
        return;
    }

    // グローバルオーバーレイ（ダッシュボード時）
    render_global_overlays(frame, app);
}

fn render_tab_content(
    frame: &mut Frame,
    app: &App,
    tab: &awsui::tab::Tab,
    spinner_tick: usize,
    area: ratatui::layout::Rect,
) {
    match app.current_view() {
        Some(View::Ec2List) => awsui::tui::views::ec2_list::render(frame, app, spinner_tick, area),
        Some(View::Ec2Detail) => awsui::tui::views::ec2_detail::render(frame, app, area),
        Some(View::EcrList) => {
            if let awsui::tab::ServiceData::Ecr {
                filtered_repositories,
                ..
            } = &tab.data
            {
                awsui::tui::views::ecr_list::render(
                    frame,
                    filtered_repositories,
                    tab.selected_index,
                    &tab.filter_input,
                    &tab.mode,
                    tab.loading,
                    spinner_tick,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    area,
                );
            }
        }
        Some(View::EcrDetail) => {
            if let awsui::tab::ServiceData::Ecr {
                filtered_repositories,
                images,
                ..
            } = &tab.data
                && let Some(repo) = filtered_repositories.get(tab.selected_index)
            {
                awsui::tui::views::ecr_detail::render(
                    frame,
                    repo,
                    images,
                    tab.detail_tag_index,
                    tab.loading,
                    spinner_tick,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    area,
                );
            }
        }
        Some(View::EcsList) => {
            if let awsui::tab::ServiceData::Ecs {
                filtered_clusters, ..
            } = &tab.data
            {
                awsui::tui::views::ecs_list::render(
                    frame,
                    filtered_clusters,
                    tab.selected_index,
                    &tab.filter_input,
                    &tab.mode,
                    tab.loading,
                    area,
                );
            }
        }
        Some(View::EcsDetail) => {
            if let awsui::tab::ServiceData::Ecs {
                filtered_clusters,
                services,
                ..
            } = &tab.data
                && let Some(cluster) = filtered_clusters.get(tab.selected_index)
            {
                awsui::tui::views::ecs_detail::render(
                    frame,
                    cluster,
                    services,
                    tab.detail_tag_index,
                    tab.loading,
                    area,
                );
            }
        }
        Some(View::S3List) => {
            if let awsui::tab::ServiceData::S3 {
                filtered_buckets, ..
            } = &tab.data
            {
                awsui::tui::views::s3_list::render(
                    frame,
                    filtered_buckets,
                    tab.selected_index,
                    &tab.filter_input,
                    &tab.mode,
                    tab.loading,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    spinner_tick,
                    area,
                );
            }
        }
        Some(View::S3Detail) => {
            if let awsui::tab::ServiceData::S3 {
                objects,
                selected_bucket,
                current_prefix,
                ..
            } = &tab.data
                && let Some(bucket_name) = selected_bucket
            {
                awsui::tui::views::s3_detail::render(
                    frame,
                    bucket_name,
                    objects,
                    current_prefix,
                    tab.detail_tag_index,
                    tab.loading,
                    spinner_tick,
                    area,
                );
            }
        }
        Some(View::VpcList) => {
            if let awsui::tab::ServiceData::Vpc { filtered_vpcs, .. } = &tab.data {
                awsui::tui::views::vpc_list::render(
                    frame,
                    filtered_vpcs,
                    tab.selected_index,
                    &tab.filter_input,
                    &tab.mode,
                    tab.loading,
                    spinner_tick,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    area,
                );
            }
        }
        Some(View::VpcDetail) => {
            if let awsui::tab::ServiceData::Vpc {
                filtered_vpcs,
                subnets,
                ..
            } = &tab.data
                && let Some(vpc) = filtered_vpcs.get(tab.selected_index)
            {
                awsui::tui::views::vpc_detail::render(
                    frame,
                    vpc,
                    subnets,
                    tab.detail_tag_index,
                    tab.loading,
                    spinner_tick,
                    area,
                );
            }
        }
        Some(View::SecretsList) => {
            if let awsui::tab::ServiceData::Secrets {
                filtered_secrets, ..
            } = &tab.data
            {
                awsui::tui::views::secrets_list::render(
                    frame,
                    filtered_secrets,
                    tab.selected_index,
                    &tab.filter_input,
                    &tab.mode,
                    tab.loading,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    spinner_tick,
                    area,
                );
            }
        }
        Some(View::SecretsDetail) => {
            if let awsui::tab::ServiceData::Secrets {
                detail,
                detail_tab,
                value_visible,
                ..
            } = &tab.data
                && let Some(detail) = detail
            {
                awsui::tui::views::secrets_detail::render(
                    frame,
                    detail,
                    tab.detail_tag_index,
                    detail_tab,
                    *value_visible,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    area,
                );
            }
        }
        Some(View::ServiceSelect) | None => {}
    }
}

fn render_tab_overlays(frame: &mut Frame, tab: &awsui::tab::Tab) {
    match &tab.mode {
        Mode::Confirm(action) => {
            let msg = match action {
                ConfirmAction::Start(id) => format!("Start instance {}?", id),
                ConfirmAction::Stop(id) => format!("Stop instance {}?", id),
                ConfirmAction::Reboot(id) => format!("Reboot instance {}?", id),
            };
            let dialog = ConfirmDialog::new(&msg);
            frame.render_widget(dialog, frame.area());
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

fn render_global_overlays(frame: &mut Frame, app: &App) {
    // サービスピッカーオーバーレイ
    if let Some(picker) = &app.service_picker {
        let widget = awsui::tui::components::service_picker::ServicePicker::new(picker);
        frame.render_widget(widget, frame.area());
    }

    if let Some(msg) = &app.message {
        let dialog = MessageDialog::new(msg);
        frame.render_widget(dialog, frame.area());
    }
    if app.show_help
        && let Some(view) = app.current_view()
    {
        let help = HelpPopup::new(&view);
        frame.render_widget(help, frame.area());
    }
}

fn load_instances(tx: &mpsc::Sender<AppEvent>, client: Arc<dyn Ec2Client>, tab_id: TabId) {
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

async fn handle_side_effects(
    app: &mut App,
    clients: &mut Clients,
    confirmed: Option<ConfirmAction>,
    prev_view: &Option<View>,
    prev_tab_id: Option<TabId>,
) {
    let current_view = app.current_view();

    // ダッシュボードに戻った場合はクライアントをクリア
    // ダッシュボードに戻った場合（以前タブにいた）はクライアントをクリア
    if app.show_dashboard && prev_view.is_some() {
        clients.clear();
    }

    let Some(tab_id) = app.active_tab().map(|t| t.id) else {
        return;
    };

    let tab_loading = app.active_tab().map(|t| t.loading).unwrap_or(false);
    let tab_view = app
        .active_tab()
        .map(|t| t.tab_view)
        .unwrap_or(awsui::tab::TabView::List);

    // 新しいタブが作成された場合 → クライアント作成 + データ読み込み
    // - prev_view.is_none(): 最初のタブ
    // - prev_view == ServiceSelect: ダッシュボードからタブ作成
    // - prev_tab_id != tab_id: ピッカーから新規タブ作成
    if tab_loading
        && tab_view == awsui::tab::TabView::List
        && (prev_view.is_none()
            || matches!(prev_view, Some(View::ServiceSelect))
            || prev_tab_id != Some(tab_id))
    {
        create_client_and_load(app, clients, tab_id).await;
        return;
    }

    // ディテールビューに遷移した場合 → 詳細データ読み込み
    if tab_loading && tab_view == awsui::tab::TabView::Detail {
        let was_list = prev_view.as_ref().is_some_and(is_list_view);
        let was_same_detail = prev_view.as_ref().is_some_and(is_detail_view);

        if was_list {
            load_detail_data(app, clients, tab_id);
            return;
        }

        // S3プレフィックスナビゲーション（S3Detail → S3Detail）
        if was_same_detail && matches!(current_view, Some(View::S3Detail)) {
            load_s3_objects(app, clients, tab_id);
            return;
        }

        // ナビゲーションリンク（EC2 Detail → VPC Detail）
        if was_same_detail && matches!(current_view, Some(View::VpcDetail)) {
            handle_navigation_link(app, clients, tab_id).await;
            return;
        }

        // シークレット値取得（SecretsDetail → SecretsDetail, loading=true）
        if was_same_detail && matches!(current_view, Some(View::SecretsDetail)) {
            load_secret_value(app, clients, tab_id);
            return;
        }
    }

    // リフレッシュ（既にリストビューにいて loading が true の場合）
    if tab_loading
        && confirmed.is_none()
        && tab_view == awsui::tab::TabView::List
        && prev_view
            .as_ref()
            .is_some_and(|v| current_view.as_ref() == Some(v))
    {
        refresh_list_data(app, clients, tab_id);
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
                        Ok(()) => AppEvent::TabEvent(
                            tab_id,
                            TabEvent::ActionCompleted(Ok(format!("Instance {} started", id))),
                        ),
                        Err(e) => AppEvent::TabEvent(tab_id, TabEvent::ActionCompleted(Err(e))),
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
                            TabEvent::ActionCompleted(Ok(format!("Instance {} stopped", id))),
                        ),
                        Err(e) => AppEvent::TabEvent(tab_id, TabEvent::ActionCompleted(Err(e))),
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
                            TabEvent::ActionCompleted(Ok(format!("Instance {} rebooted", id))),
                        ),
                        Err(e) => AppEvent::TabEvent(tab_id, TabEvent::ActionCompleted(Err(e))),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }

    // フォーム送信 → CRUD API呼び出し
    if let Some(form_ctx) = app.pending_form.take() {
        handle_form_side_effect(app, clients, form_ctx, tab_id);
    }

    // 危険操作確認 → CRUD API呼び出し
    if let Some(danger_action) = app.pending_danger_action.take() {
        handle_danger_side_effect(app, clients, danger_action, tab_id);
    }
}

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
            Err(e) => {
                if let Some(t) = app.find_tab_mut(tab_id) {
                    t.loading = false;
                }
                app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
            }
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
            Err(e) => {
                if let Some(t) = app.find_tab_mut(tab_id) {
                    t.loading = false;
                }
                app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
            }
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
            Err(e) => {
                if let Some(t) = app.find_tab_mut(tab_id) {
                    t.loading = false;
                }
                app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
            }
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
            Err(e) => {
                if let Some(t) = app.find_tab_mut(tab_id) {
                    t.loading = false;
                }
                app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
            }
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
            Err(e) => {
                if let Some(t) = app.find_tab_mut(tab_id) {
                    t.loading = false;
                }
                app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
            }
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
                Err(e) => {
                    if let Some(t) = app.find_tab_mut(tab_id) {
                        t.loading = false;
                    }
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                }
            }
        }
    }
}

fn load_detail_data(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };

    match &tab.data {
        awsui::tab::ServiceData::Ecr {
            filtered_repositories,
            ..
        } => {
            if let Some(repo) = filtered_repositories.get(tab.selected_index)
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
        awsui::tab::ServiceData::Ecs {
            filtered_clusters, ..
        } => {
            if let Some(cluster) = filtered_clusters.get(tab.selected_index)
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
                let tx = app.event_tx.clone();
                let c = client.clone();
                let bucket = selected_bucket.clone().unwrap_or_default();
                let prefix = if current_prefix.is_empty() {
                    None
                } else {
                    Some(current_prefix.clone())
                };
                tokio::spawn(async move {
                    let result = c.list_objects(&bucket, prefix).await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::ObjectsLoaded(result)))
                        .await;
                });
            }
        }
        awsui::tab::ServiceData::Vpc { filtered_vpcs, .. } => {
            if let Some(vpc) = filtered_vpcs.get(tab.selected_index)
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
        awsui::tab::ServiceData::Secrets {
            filtered_secrets, ..
        } => {
            if let Some(secret) = filtered_secrets.get(tab.selected_index)
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

fn load_secret_value(app: &App, clients: &Clients, tab_id: TabId) {
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

fn load_s3_objects(app: &App, clients: &Clients, tab_id: TabId) {
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
        let tx = app.event_tx.clone();
        let c = client.clone();
        let bucket = selected_bucket.clone().unwrap_or_default();
        let prefix = if current_prefix.is_empty() {
            None
        } else {
            Some(current_prefix.clone())
        };
        tokio::spawn(async move {
            let result = c.list_objects(&bucket, prefix).await;
            let _ = tx
                .send(AppEvent::TabEvent(tab_id, TabEvent::ObjectsLoaded(result)))
                .await;
        });
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
                        tab.tab_view = match (entry.view.clone(), tab.service) {
                            (View::Ec2List, _) => awsui::tab::TabView::List,
                            (View::Ec2Detail, _) => awsui::tab::TabView::Detail,
                            _ => awsui::tab::TabView::Detail,
                        };
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

fn refresh_list_data(app: &App, clients: &Clients, tab_id: TabId) {
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

fn handle_form_side_effect(
    app: &mut App,
    clients: &Clients,
    form_ctx: awsui::app::FormContext,
    tab_id: TabId,
) {
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
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Bucket '{}' created", bucket_name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
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
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Secret '{}' created", name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::UpdateSecretValue => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                let secret_id = app
                    .active_tab()
                    .and_then(|t| {
                        if let awsui::tab::ServiceData::Secrets { detail, .. } = &t.data {
                            detail.as_ref().map(|d| d.arn.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let new_value = values[0].clone();
                tokio::spawn(async move {
                    let result = c.update_secret_value(&secret_id, &new_value).await;
                    let event = match result {
                        Ok(()) => {
                            AppEvent::CrudCompleted(tab_id, Ok("Secret value updated".to_string()))
                        }
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
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
    tab_id: TabId,
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
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Instance {} terminated", id)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
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
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Bucket '{}' deleted", name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
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
                        Ok(()) => {
                            AppEvent::CrudCompleted(tab_id, Ok(format!("Object '{}' deleted", key)))
                        }
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
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
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Secret '{}' deleted", name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }
}

/// CRUD完了後にリストをリフレッシュする
fn trigger_refresh(app: &App, clients: &Clients) {
    let Some(tab) = app.active_tab() else {
        return;
    };
    let tab_id = tab.id;
    let tx = app.event_tx.clone();

    match app.current_view() {
        Some(View::Ec2List) => {
            if let Some(client) = &clients.ec2 {
                load_instances(&tx, client.clone(), tab_id);
            }
        }
        Some(View::S3List) => {
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
        Some(View::S3Detail) => {
            if let awsui::tab::ServiceData::S3 {
                selected_bucket,
                current_prefix,
                ..
            } = &tab.data
                && let Some(client) = &clients.s3
            {
                let c = client.clone();
                let bucket = selected_bucket.clone().unwrap_or_default();
                let prefix = if current_prefix.is_empty() {
                    None
                } else {
                    Some(current_prefix.clone())
                };
                tokio::spawn(async move {
                    let result = c.list_objects(&bucket, prefix).await;
                    let _ = tx
                        .send(AppEvent::TabEvent(tab_id, TabEvent::ObjectsLoaded(result)))
                        .await;
                });
            }
        }
        Some(View::SecretsList) => {
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
        Some(View::SecretsDetail) => {
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
