mod clients;
mod effects;
mod loader;
mod rendering;

use awsui::app::{App, SideEffect};
use awsui::cli::{Cli, DeletePermissions};
use awsui::config;
use awsui::event::{AppEvent, TabEvent};
use awsui::sso::{self, SsoTokenStatus};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. CLI引数パース
    let cli = Cli::parse();
    let delete_permissions =
        DeletePermissions::from_cli(cli.allow_delete.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

    // 1. SSOプロファイル読み込み + App初期化
    let mut app = if let Some(profile_name) = cli.profile {
        // --profile が指定されていればプロファイル選択画面をスキップ
        let profiles = config::load_sso_profiles().unwrap_or_default();
        let region = config::get_region_for_profile(&profiles, &profile_name);

        // SSO トークンチェック + login（同期的に実行）
        if let Some(profile) = profiles.iter().find(|p| p.name == profile_name) {
            match sso::check_sso_token(profile) {
                SsoTokenStatus::Valid => {}
                SsoTokenStatus::Expired | SsoTokenStatus::NotFound => {
                    let status = std::process::Command::new("aws")
                        .args(["sso", "login", "--profile", &profile_name])
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

        App::with_delete_permissions(profile_name, region, delete_permissions)
    } else {
        // プロファイル選択画面から開始
        let profiles = config::load_sso_profiles()?;
        if profiles.is_empty() {
            eprintln!("No SSO profiles found in ~/.aws/config");
            return Ok(());
        }
        App::new_with_profile_selector(profiles, delete_permissions)
    };

    // 5. ターミナル初期化
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 6. メインループ
    let mut event_stream = crossterm::event::EventStream::new();
    let mut render_interval = tokio::time::interval(std::time::Duration::from_millis(16));
    let mut clients = clients::Clients::default();
    let mut spinner_tick: usize = 0;
    let mut log_poll_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut sso_login_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut sso_cancel_tx: Option<tokio::sync::oneshot::Sender<()>> = None;

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
                    loader::load_instances(&app.event_tx, client.clone(), tab.id);
                }
                if needs_crud_refresh {
                    loader::trigger_refresh(&app, &clients);
                }
                // ログ初回取得（EcsLogConfigsLoaded後にlog_stateが設定されloading=trueの場合）
                if let Some(tab) = app.active_tab()
                    && tab.loading
                    && let awsui::tab::ServiceData::Ecs { nav_level: Some(awsui::tab::EcsNavLevel::LogView { log_state, .. }), .. } = &tab.data
                {
                    let log_group = log_state.log_group.clone();
                    let log_stream = log_state.log_stream.clone();
                    let next_token = log_state.next_forward_token.clone();
                    let tid = tab.id;
                    effects::fetch_log_events(&mut app, &mut clients, tid, &log_group, &log_stream, next_token).await;
                }
                // ログイベント受信後にポーリング継続を管理
                effects::manage_log_polling(&app, &clients, &mut log_poll_handle);
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

                    // SSO loginプロセス起動
                    if let SideEffect::StartSsoLogin { profile_name, region } = &side_effect {
                        let (handle, cancel) = effects::start_sso_login(
                            app.event_tx.clone(),
                            profile_name.clone(),
                            region.clone(),
                        );
                        sso_login_handle = Some(handle);
                        sso_cancel_tx = Some(cancel);
                    } else if let SideEffect::SsmConnect { instance_id } = &side_effect {
                        effects::run_ssm_connect(&mut terminal, instance_id, &mut app);
                    } else if let SideEffect::EcsExec { cluster_arn, task_arn, container_name } = &side_effect {
                        effects::run_ecs_exec(&mut terminal, cluster_arn, task_arn, container_name, &mut app);
                    } else {
                        effects::handle_side_effects(&mut app, &mut clients, side_effect, &prev_view, prev_tab_id, &action_clone).await;
                    }

                    // SSO loginキャンセル時のプロセス停止
                    if action_clone == awsui::action::Action::CancelSsoLogin {
                        effects::cancel_sso_login(&mut sso_login_handle, &mut sso_cancel_tx);
                    }

                    // ログポーリング管理
                    effects::manage_log_polling(&app, &clients, &mut log_poll_handle);
                }
            }
            _ = render_interval.tick() => {
                let is_loading = app.active_tab().map(|t| t.loading).unwrap_or(false);
                let is_sso_logging_in = app.profile_selector.as_ref().is_some_and(|ps| ps.logging_in);
                if is_loading || is_sso_logging_in {
                    spinner_tick = spinner_tick.wrapping_add(1);
                }
                terminal.draw(|frame| rendering::render(frame, &app, spinner_tick))?;
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
