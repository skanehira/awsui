use std::io::Cursor;
use std::sync::Arc;

use awsui::app::{App, ConfirmAction, Mode, SideEffect};
use awsui::aws::client::Ec2Client;
use awsui::aws::ecr_client::EcrClient;
use awsui::aws::ecs_client::EcsClient;
use awsui::aws::logs_client::LogsClient;
use awsui::aws::s3_client::S3Client;
use awsui::aws::secrets_client::SecretsClient;
use awsui::aws::vpc_client::VpcClient;
#[cfg(not(feature = "mock-data"))]
use awsui::aws::{
    client::AwsEc2Client, ecr_client::AwsEcrClient, ecs_client::AwsEcsClient,
    logs_client::AwsLogsClient, s3_client::AwsS3Client, secrets_client::AwsSecretsClient,
    vpc_client::AwsVpcClient,
};
use awsui::cli::{Cli, DeletePermissions};
use awsui::config;
use awsui::event::{AppEvent, TabEvent};
use awsui::service::ServiceKind;
use awsui::sso::{self, SsoTokenStatus};
use awsui::tab::TabId;
use awsui::tab::TabView;
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
    logs: Option<Arc<dyn LogsClient>>,
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
            logs: None,
        }
    }

    fn clear(&mut self) {
        self.ec2 = None;
        self.ecr = None;
        self.ecs = None;
        self.s3 = None;
        self.vpc = None;
        self.secrets = None;
        self.logs = None;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. CLI引数パース
    let cli = Cli::parse();
    let delete_permissions =
        DeletePermissions::from_cli(cli.allow_delete.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

    // 1. SSOプロファイル読み込み + プロファイル選択
    let (selected_profile, region) = if let Some(profile_name) = cli.profile {
        // --profile が指定されていればインタラクティブ選択をスキップ
        let profiles = config::load_sso_profiles().unwrap_or_default();
        let region = config::get_region_for_profile(&profiles, &profile_name);
        (profile_name, region)
    } else {
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

        let region = config::get_region_for_profile(&profiles, &selected_profile);
        (selected_profile, region)
    };

    // 4. リージョン取得 + App初期化
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
    let mut log_poll_handle: Option<tokio::task::JoinHandle<()>> = None;

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
                // ログ初回取得（EcsLogConfigsLoaded後にlog_stateが設定されloading=trueの場合）
                if let Some(tab) = app.active_tab()
                    && tab.loading
                    && let awsui::tab::ServiceData::Ecs { log_state: Some(state), .. } = &tab.data
                {
                    let log_group = state.log_group.clone();
                    let log_stream = state.log_stream.clone();
                    let next_token = state.next_forward_token.clone();
                    let tid = tab.id;
                    fetch_log_events(&mut app, &mut clients, tid, &log_group, &log_stream, next_token).await;
                }
                // ログイベント受信後にポーリング継続を管理
                manage_log_polling(&app, &clients, &mut log_poll_handle);
            }
            maybe_event = futures::StreamExt::next(&mut event_stream) => {
                if let Some(Ok(crossterm::event::Event::Key(key))) = maybe_event
                    && key.kind == crossterm::event::KeyEventKind::Press
                {
                    let prev_view = app.current_view();
                    let prev_tab_id = app.active_tab().map(|t| t.id);
                    let action = awsui::tui::input::handle_key(&app, key);
                    let action_clone = action.clone();
                    let side_effect = app.dispatch(action);

                    handle_side_effects(&mut app, &mut clients, side_effect, &prev_view, prev_tab_id, &action_clone).await;

                    // ログポーリング管理
                    manage_log_polling(&app, &clients, &mut log_poll_handle);
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
        Some((ServiceKind::Ec2, TabView::List)) => {
            awsui::tui::views::ec2_list::render(frame, app, spinner_tick, area)
        }
        Some((ServiceKind::Ec2, TabView::Detail)) => {
            awsui::tui::views::ec2_detail::render(frame, app, area)
        }
        Some((ServiceKind::Ecr, TabView::List)) => {
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
        Some((ServiceKind::Ecr, TabView::Detail)) => {
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
        Some((ServiceKind::Ecs, TabView::List)) => {
            if let awsui::tab::ServiceData::Ecs {
                filtered_clusters, ..
            } = &tab.data
            {
                let props = awsui::tui::views::ecs_list::EcsListProps {
                    clusters: filtered_clusters,
                    selected_index: tab.selected_index,
                    filter_input: &tab.filter_input,
                    mode: &tab.mode,
                    loading: tab.loading,
                    spinner_tick,
                };
                awsui::tui::views::ecs_list::render(frame, &props, area);
            }
        }
        Some((ServiceKind::Ecs, TabView::Detail)) => {
            if let awsui::tab::ServiceData::Ecs {
                filtered_clusters,
                services,
                selected_service_index,
                tasks,
                selected_task_index,
                log_state,
                ..
            } = &tab.data
                && let Some(cluster) = filtered_clusters.get(tab.selected_index)
            {
                // ログビュー表示中
                if let Some(log_state) = log_state {
                    awsui::tui::views::ecs_log::render(
                        frame,
                        log_state,
                        tab.loading,
                        spinner_tick,
                        &tab.mode,
                        tab.filter_input.value(),
                        area,
                    );
                } else if let Some(task_idx) = selected_task_index
                    && let Some(task) = tasks.get(*task_idx)
                {
                    // タスク詳細
                    awsui::tui::views::ecs_task_detail::render(frame, task, area);
                } else if let Some(svc_idx) = selected_service_index
                    && let Some(service) = services.get(*svc_idx)
                {
                    // サービス詳細（タスク一覧付き）
                    awsui::tui::views::ecs_service_detail::render(
                        frame,
                        service,
                        tasks,
                        tab.detail_tag_index,
                        tab.loading,
                        spinner_tick,
                        area,
                    );
                } else {
                    // クラスター詳細（サービス一覧）
                    awsui::tui::views::ecs_detail::render(
                        frame,
                        cluster,
                        services,
                        tab.detail_tag_index,
                        tab.loading,
                        spinner_tick,
                        area,
                    );
                }
            }
        }
        Some((ServiceKind::S3, TabView::List)) => {
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
        Some((ServiceKind::S3, TabView::Detail)) => {
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
        Some((ServiceKind::Vpc, TabView::List)) => {
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
        Some((ServiceKind::Vpc, TabView::Detail)) => {
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
        Some((ServiceKind::SecretsManager, TabView::List)) => {
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
        Some((ServiceKind::SecretsManager, TabView::Detail)) => {
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
        None => {}
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
        Mode::ContainerSelect { names, selected } => {
            let popup = awsui::tui::components::dialog::centered_rect(
                50,
                (names.len() as u16 + 5).min(20),
                frame.area(),
            );
            frame.render_widget(ratatui::widgets::Clear, popup);
            let block = ratatui::widgets::Block::default()
                .title(" Select Container ")
                .borders(ratatui::widgets::Borders::ALL)
                .style(awsui::tui::theme::active());
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            let selector =
                awsui::tui::components::list_selector::ListSelector::new("", names, *selected);
            frame.render_widget(selector, inner);
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
        let help = HelpPopup::new(view);
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
    side_effect: SideEffect,
    prev_view: &Option<(ServiceKind, TabView)>,
    prev_tab_id: Option<TabId>,
    action_clone: &awsui::action::Action,
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

    // ShowLogs → タスク定義のログ設定取得（他の早期リターンより先に処理）
    if matches!(action_clone, awsui::action::Action::ShowLogs) && tab_loading {
        load_ecs_log_configs(app, clients, tab_id);
        return;
    }

    // ログ初回取得（log_stateがありloading=trueの場合、ContainerSelectConfirm後など）
    if tab_loading
        && let Some(tab) = app.find_tab(tab_id)
        && let awsui::tab::ServiceData::Ecs {
            log_state: Some(state),
            ..
        } = &tab.data
    {
        let log_group = state.log_group.clone();
        let log_stream = state.log_stream.clone();
        let next_token = state.next_forward_token.clone();
        fetch_log_events(app, clients, tab_id, &log_group, &log_stream, next_token).await;
        return;
    }

    // 新しいタブが作成された場合 → クライアント作成 + データ読み込み
    // - prev_view.is_none(): 最初のタブ
    // - prev_view == ServiceSelect: ダッシュボードからタブ作成
    // - prev_tab_id != tab_id: ピッカーから新規タブ作成
    if tab_loading
        && tab_view == awsui::tab::TabView::List
        && (prev_view.is_none() || prev_tab_id != Some(tab_id))
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

        // ECSタスク読み込み（サービス詳細遷移時）
        if was_same_detail && matches!(current_view, Some((ServiceKind::Ecs, TabView::Detail))) {
            load_ecs_tasks(app, clients, tab_id);
            return;
        }

        // S3プレフィックスナビゲーション（S3Detail → S3Detail）
        if was_same_detail && matches!(current_view, Some((ServiceKind::S3, TabView::Detail))) {
            load_s3_objects(app, clients, tab_id);
            return;
        }

        // ナビゲーションリンク（EC2 Detail → VPC Detail）
        if was_same_detail && matches!(current_view, Some((ServiceKind::Vpc, TabView::Detail))) {
            handle_navigation_link(app, clients, tab_id).await;
            return;
        }

        // シークレット値取得（SecretsDetail → SecretsDetail, loading=true）
        if was_same_detail
            && matches!(
                current_view,
                Some((ServiceKind::SecretsManager, TabView::Detail))
            )
        {
            load_secret_value(app, clients, tab_id);
            return;
        }
    }

    // リフレッシュ（既にリストビューにいて loading が true の場合）
    if tab_loading
        && matches!(side_effect, SideEffect::None)
        && tab_view == awsui::tab::TabView::List
        && prev_view
            .as_ref()
            .is_some_and(|v| current_view.as_ref() == Some(v))
    {
        refresh_list_data(app, clients, tab_id);
        return;
    }

    // 副作用の処理
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
            handle_form_side_effect(app, clients, form_ctx, tab_id);
        }
        SideEffect::DangerAction(danger_action) => {
            handle_danger_side_effect(app, clients, danger_action, tab_id);
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

fn load_ecs_tasks(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    if let awsui::tab::ServiceData::Ecs {
        filtered_clusters,
        services,
        selected_service_index,
        ..
    } = &tab.data
        && let Some(svc_idx) = selected_service_index
        && let Some(service) = services.get(*svc_idx)
        && let Some(cluster) = filtered_clusters.get(tab.selected_index)
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

fn is_list_view(view: &(ServiceKind, TabView)) -> bool {
    view.1 == TabView::List
}

fn is_detail_view(view: &(ServiceKind, TabView)) -> bool {
    view.1 == TabView::Detail
}

/// ECSタスク定義のログ設定を取得する
fn load_ecs_log_configs(app: &App, clients: &Clients, tab_id: TabId) {
    let Some(tab) = app.find_tab(tab_id) else {
        return;
    };
    if let awsui::tab::ServiceData::Ecs {
        tasks,
        selected_task_index,
        ..
    } = &tab.data
        && let Some(task_idx) = selected_task_index
        && let Some(task) = tasks.get(*task_idx)
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

/// CloudWatch Logsからログイベントを取得する
async fn fetch_log_events(
    app: &mut App,
    clients: &mut Clients,
    tab_id: TabId,
    log_group: &str,
    log_stream: &str,
    next_token: Option<String>,
) {
    // Logsクライアントがない場合は作成
    if clients.logs.is_none() {
        #[cfg(feature = "mock-data")]
        {
            clients.logs = Some(Arc::new(awsui::aws::mock_clients::MockLogsClientImpl));
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
            match AwsLogsClient::new(profile_name, &region).await {
                Ok(client) => {
                    clients.logs = Some(Arc::new(client));
                }
                Err(e) => {
                    if let Some(tab) = app.find_tab_mut(tab_id) {
                        tab.loading = false;
                    }
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                    return;
                }
            }
        }
    }

    if let Some(client) = &clients.logs {
        let tx = app.event_tx.clone();
        let c = client.clone();
        let group = log_group.to_string();
        let stream = log_stream.to_string();
        tokio::spawn(async move {
            let result = c.get_log_events(&group, &stream, next_token).await;
            let _ = tx
                .send(AppEvent::TabEvent(
                    tab_id,
                    TabEvent::EcsLogEventsLoaded(result),
                ))
                .await;
        });
    }
}

/// ログポーリングを管理する（ログビュー表示中は2秒間隔でポーリング、非表示時は停止）
fn manage_log_polling(
    app: &App,
    clients: &Clients,
    log_poll_handle: &mut Option<tokio::task::JoinHandle<()>>,
) {
    let should_poll = app.active_tab().is_some_and(|tab| {
        if let awsui::tab::ServiceData::Ecs { log_state, .. } = &tab.data {
            log_state.is_some() && !tab.loading
        } else {
            false
        }
    });

    if should_poll {
        // 既にポーリング中なら何もしない
        if log_poll_handle.as_ref().is_some_and(|h| !h.is_finished()) {
            return;
        }

        // ログ情報を取得
        let Some(tab) = app.active_tab() else {
            return;
        };
        let tab_id = tab.id;
        let Some((log_group, log_stream, next_token)) = (|| {
            if let awsui::tab::ServiceData::Ecs {
                log_state: Some(state),
                ..
            } = &tab.data
            {
                return Some((
                    state.log_group.clone(),
                    state.log_stream.clone(),
                    state.next_forward_token.clone(),
                ));
            }
            None
        })() else {
            return;
        };

        let Some(client) = clients.logs.clone() else {
            return;
        };
        let tx = app.event_tx.clone();

        *log_poll_handle = Some(tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let result = client
                .get_log_events(&log_group, &log_stream, next_token)
                .await;
            let _ = tx
                .send(AppEvent::TabEvent(
                    tab_id,
                    TabEvent::EcsLogEventsLoaded(result),
                ))
                .await;
        }));
    } else {
        // ログビューでなくなったらポーリング停止
        if let Some(handle) = log_poll_handle.take() {
            handle.abort();
        }
    }
}
