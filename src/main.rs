use std::sync::Arc;

use awsui::app::{App, ConfirmAction, Mode};
use awsui::aws::client::{AwsEc2Client, Ec2Client};
use awsui::config;
use awsui::event::AppEvent;
use awsui::tui::components::dialog::{ConfirmDialog, MessageDialog};
use awsui::tui::components::help::HelpPopup;
use ratatui::Frame;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. SSOプロファイル読み込み
    let profiles = config::load_sso_profiles()?;
    let profile_names = config::profile_names(&profiles);

    if profile_names.is_empty() {
        eprintln!("No SSO profiles found in ~/.aws/config");
        return Ok(());
    }

    // 2. App初期化
    let mut app = App::new(profile_names.clone());

    // 3. ターミナル初期化
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 4. メインループ
    let mut event_stream = crossterm::event::EventStream::new();
    let mut render_interval = tokio::time::interval(std::time::Duration::from_millis(16));
    let mut ec2_client: Option<Arc<dyn Ec2Client>> = None;
    let mut spinner_tick: usize = 0;

    loop {
        tokio::select! {
            Some(event) = app.event_rx.recv() => {
                let needs_refresh = matches!(&event, AppEvent::ActionCompleted(Ok(_)));
                app.handle_event(event);
                if needs_refresh
                    && let Some(client) = &ec2_client {
                    load_instances(&app.event_tx, client.clone());
                }
            }
            maybe_event = futures::StreamExt::next(&mut event_stream) => {
                if let Some(Ok(crossterm::event::Event::Key(key))) = maybe_event
                    && key.kind == crossterm::event::KeyEventKind::Press
                {
                    let action = awsui::tui::input::handle_key(&app, key);
                    let confirmed = app.dispatch(action);

                    handle_side_effects(&mut app, &profiles, &mut ec2_client, confirmed).await;
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
        awsui::app::View::ProfileSelect => awsui::tui::views::profile_select::render(frame, app),
        awsui::app::View::Ec2List => awsui::tui::views::ec2_list::render(frame, app, spinner_tick),
        awsui::app::View::Ec2Detail => awsui::tui::views::ec2_detail::render(frame, app),
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
    ec2_client: &mut Option<Arc<dyn Ec2Client>>,
    confirmed: Option<ConfirmAction>,
) {
    // Enter on ProfileSelect → EC2クライアント作成 + インスタンス読み込み
    if app.view == awsui::app::View::Ec2List
        && ec2_client.is_none()
        && let Some(profile_name) = &app.profile
    {
        let region = config::get_region_for_profile(profiles, profile_name)
            .unwrap_or_else(|| "ap-northeast-1".to_string());
        app.region = Some(region.clone());
        app.loading = true;

        match AwsEc2Client::new(profile_name, &region).await {
            Ok(client) => {
                let client: Arc<dyn Ec2Client> = Arc::new(client);
                load_instances(&app.event_tx, client.clone());
                *ec2_client = Some(client);
            }
            Err(e) => {
                app.loading = false;
                app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
            }
        }
    }

    // ProfileSelectに戻った場合はクライアントをクリア
    if app.view == awsui::app::View::ProfileSelect {
        *ec2_client = None;
    }

    // Refresh
    if app.loading
        && confirmed.is_none()
        && let Some(client) = ec2_client
    {
        load_instances(&app.event_tx, client.clone());
    }

    // ConfirmYes → API呼び出し
    if let Some(action) = confirmed
        && let Some(client) = ec2_client.clone()
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
}
