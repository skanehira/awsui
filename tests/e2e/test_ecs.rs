use crate::common::{TestHarness, spawn_app};

/// ECSサービス詳細画面まで遷移するヘルパー
/// Dashboard → ECS (index 2) → production → web-service → service detail
fn navigate_to_service_detail(h: &mut TestHarness) {
    h.select_service(2); // ECS = index 2
    h.wait_for_text("production").unwrap();
    h.send_enter().unwrap(); // クラスター詳細へ
    h.wait_for_text("web-service").unwrap();
    h.send_enter().unwrap(); // サービス詳細へ
    h.wait_for_text("abc123").unwrap(); // タスクロード完了待ち
}

#[test]
fn ecs_list_shows_clusters_when_selected() {
    let mut h = spawn_app(120, 24);
    h.select_service(2); // ECS = index 2
    h.wait_for_text("production")
        .expect("ECS list should show mock clusters");
    let contents = h.screen_contents();
    assert!(
        contents.contains("ECS Clusters"),
        "Should show ECS Clusters header, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecs_detail_shows_services_when_entered() {
    let mut h = spawn_app(120, 24);
    h.select_service(2);
    h.wait_for_text("production").unwrap();
    // productionクラスターを選択
    h.send_enter().unwrap();
    // サービス一覧のロード完了を待つ
    h.wait_for_text("web-service")
        .expect("ECS cluster detail should show services");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecs_service_detail_shows_tasks_when_entered() {
    let mut h = spawn_app(120, 24);
    h.select_service(2);
    h.wait_for_text("production").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("web-service").unwrap();
    // web-serviceを選択してサービス詳細へ
    h.send_enter().unwrap();
    // タスク一覧のロード完了を待つ
    h.wait_for_text("abc123")
        .expect("ECS service detail should show tasks");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecs_back_returns_through_levels_when_escape() {
    let mut h = spawn_app(120, 24);
    h.select_service(2);
    h.wait_for_text("production").unwrap();
    // クラスター詳細へ
    h.send_enter().unwrap();
    h.wait_for_text("web-service").unwrap();
    // サービス詳細へ
    h.send_enter().unwrap();
    h.wait_for_text("abc123").unwrap();
    // Escでサービス一覧に戻る
    h.send_esc().unwrap();
    h.wait_for_text("web-service")
        .expect("Should return to service list");
    // Escでクラスター一覧に戻る
    h.send_esc().unwrap();
    h.wait_for_text("ECS Clusters")
        .expect("Should return to cluster list");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecs_service_detail_shows_deployments_when_tab_switched() {
    let mut h = spawn_app(120, 30);
    navigate_to_service_detail(&mut h);
    // ] でDeploymentsタブに切り替え
    h.send_char(']').unwrap();
    // Deploymentsタブのヘッダーが表示されることを確認
    h.wait_for_text("Rollout State")
        .expect("Deployments tab should show table headers");
    let contents = h.screen_contents();
    assert!(
        contents.contains("PRIMARY"),
        "Deployments tab should show deployment status, got:\n{}",
        contents
    );
    assert!(
        contents.contains("COMPLETED"),
        "Deployments tab should show rollout state, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecs_force_deploy_shows_success_when_confirmed() {
    let mut h = spawn_app(120, 30);
    navigate_to_service_detail(&mut h);
    // d でforce deploy確認ダイアログを表示
    h.send_char('d').unwrap();
    h.wait_for_text("Force new deployment for web-service?")
        .expect("Should show force deploy confirm dialog");
    // y で確認
    h.send_char('y').unwrap();
    // 成功メッセージを待つ
    h.wait_for_text("Force new deployment started")
        .expect("Should show success message after force deploy");
    // メッセージを閉じる
    h.send_enter().unwrap();
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecs_scale_service_shows_success_when_submitted() {
    let mut h = spawn_app(120, 30);
    navigate_to_service_detail(&mut h);
    // s でスケールフォームを表示
    h.send_char('s').unwrap();
    h.wait_for_text("Scale Service")
        .expect("Should show scale service form dialog");
    // フォームはdesired_count=3が初期値。Enterで送信
    h.send_enter().unwrap();
    // 成功メッセージを待つ
    h.wait_for_text("Desired count updated to 3 for web-service")
        .expect("Should show success message after scale");
    // メッセージを閉じる
    h.send_enter().unwrap();
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecs_stop_task_shows_success_when_confirmed() {
    let mut h = spawn_app(120, 30);
    navigate_to_service_detail(&mut h);
    // x でタスク停止確認ダイアログを表示
    h.send_char('x').unwrap();
    h.wait_for_text("Stop task")
        .expect("Should show stop task confirm dialog");
    // y で確認
    h.send_char('y').unwrap();
    // 成功メッセージを待つ
    h.wait_for_text("stopped")
        .expect("Should show success message after stop task");
    // メッセージを閉じる
    h.send_enter().unwrap();
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
