use awsui::app::App;
use awsui::event::AppEvent;
use tokio::sync::mpsc;

/// SSM Session Manager でEC2インスタンスに接続する
/// TUIを一時停止し、aws ssm start-sessionを対話的に実行、終了後にTUIを復帰する
pub(crate) fn run_ssm_connect(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    instance_id: &str,
    app: &mut App,
) {
    let Some(profile) = &app.profile else {
        return;
    };
    let profile = profile.clone();

    // ターミナル復元
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    );

    // aws ssm start-session を対話的に実行
    let status = std::process::Command::new("aws")
        .args([
            "ssm",
            "start-session",
            "--target",
            instance_id,
            "--profile",
            &profile,
        ])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    // ターミナル再初期化
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen
    );
    let _ = terminal.clear();

    // コマンド失敗時はTUI復帰後にエラーメッセージ表示
    match status {
        Err(e) => {
            app.show_message(
                awsui::app::MessageLevel::Error,
                "SSM Connect Failed",
                format!("Failed to start SSM session: {}", e),
            );
        }
        Ok(s) if !s.success() => {
            app.show_message(
                awsui::app::MessageLevel::Error,
                "SSM Connect Failed",
                format!("SSM session exited with code: {}", s.code().unwrap_or(-1)),
            );
        }
        _ => {}
    }
}

/// ECS Execute Command でコンテナにアタッチする
/// TUIを一時停止し、aws ecs execute-commandを対話的に実行、終了後にTUIを復帰する
pub(crate) fn run_ecs_exec(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    cluster_arn: &str,
    task_arn: &str,
    container_name: &str,
    app: &mut App,
) {
    let Some(profile) = &app.profile else {
        return;
    };
    let profile = profile.clone();

    // ターミナル復元
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    );

    // aws ecs execute-command を対話的に実行
    let status = std::process::Command::new("aws")
        .args([
            "ecs",
            "execute-command",
            "--cluster",
            cluster_arn,
            "--task",
            task_arn,
            "--container",
            container_name,
            "--command",
            "/bin/sh",
            "--interactive",
            "--profile",
            &profile,
        ])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    // ターミナル再初期化
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen
    );
    let _ = terminal.clear();

    // コマンド失敗時はTUI復帰後にエラーメッセージ表示
    match status {
        Err(e) => {
            app.show_message(
                awsui::app::MessageLevel::Error,
                "ECS Exec Failed",
                format!("Failed to start ECS Exec session: {}", e),
            );
        }
        Ok(s) if !s.success() => {
            app.show_message(
                awsui::app::MessageLevel::Error,
                "ECS Exec Failed",
                format!("ECS Exec exited with code: {}", s.code().unwrap_or(-1)),
            );
        }
        _ => {}
    }
}

/// SSO loginプロセスをキャンセルする
pub(crate) fn cancel_sso_login(
    sso_login_handle: &mut Option<tokio::task::JoinHandle<()>>,
    sso_cancel_tx: &mut Option<tokio::sync::oneshot::Sender<()>>,
) {
    // キャンセル信号を送る（spawn内のselect!がchild.kill()を呼ぶ）
    if let Some(tx) = sso_cancel_tx.take() {
        let _ = tx.send(());
    }
    // JoinHandleはspawn内で自然終了するのでtakeするだけ
    sso_login_handle.take();
}

/// SSO loginプロセスをバックグラウンドで起動する
pub(crate) fn start_sso_login(
    tx: mpsc::Sender<AppEvent>,
    profile_name: String,
    region: Option<String>,
) -> (
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();

    let mut child = match tokio::process::Command::new("aws")
        .args(["sso", "login", "--profile", &profile_name])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            let tx = tx.clone();
            let handle = tokio::spawn(async move {
                let _ = tx
                    .send(AppEvent::SsoLoginCompleted(Err(
                        awsui::error::AppError::Io(e),
                    )))
                    .await;
            });
            return (handle, cancel_tx);
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let handle = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let tx_stdout = tx.clone();
        let tx_stderr = tx.clone();

        let stdout_task = tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx_stdout.send(AppEvent::SsoLoginOutput(line)).await;
                }
            }
        });

        let stderr_task = tokio::spawn(async move {
            if let Some(stderr) = stderr {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx_stderr.send(AppEvent::SsoLoginOutput(line)).await;
                }
            }
        });

        tokio::select! {
            status = child.wait() => {
                let _ = stdout_task.await;
                let _ = stderr_task.await;

                let result = match status {
                    Ok(s) if s.success() => Ok((profile_name, region)),
                    Ok(s) => Err(awsui::error::AppError::AwsApi(format!(
                        "aws sso login exited with status: {}",
                        s
                    ))),
                    Err(e) => Err(awsui::error::AppError::Io(e)),
                };
                let _ = tx.send(AppEvent::SsoLoginCompleted(result)).await;
            }
            _ = cancel_rx => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
            }
        }
    });

    (handle, cancel_tx)
}
