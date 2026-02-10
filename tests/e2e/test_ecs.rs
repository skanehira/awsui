use crate::common::spawn_app;

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
