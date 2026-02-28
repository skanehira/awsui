use std::time::Duration;

use crate::common::spawn_app;

#[test]
fn ecr_list_shows_repositories_when_selected() {
    let mut h = spawn_app(120, 24);
    h.select_service(1); // ECR = index 1
    h.wait_for_text("myapp/web")
        .expect("ECR list should show mock repositories");
    let contents = h.screen_contents();
    assert!(
        contents.contains("ECR Repositories"),
        "Should show ECR Repositories header, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecr_detail_shows_images_when_entered() {
    let mut h = spawn_app(120, 24);
    h.select_service(1);
    h.wait_for_text("myapp/web").unwrap();
    // 最初のリポジトリを選択して詳細へ
    h.send_enter().unwrap();
    h.wait_for_text("sha256:")
        .expect("ECR detail should show image digests");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecr_back_returns_to_list_when_escape() {
    let mut h = spawn_app(120, 24);
    h.select_service(1);
    h.wait_for_text("myapp/web").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("sha256:").unwrap();
    // Escで一覧に戻る
    h.send_esc().unwrap();
    h.wait_for_text("ECR Repositories")
        .expect("Should return to ECR list");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecr_detail_shows_scan_tab_when_switched() {
    let mut h = spawn_app(120, 24);
    h.select_service(1);
    h.wait_for_text("myapp/web").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("sha256:").unwrap();
    // ] でScanタブに切り替え
    h.send_char(']').unwrap();
    std::thread::sleep(Duration::from_millis(500));
    // Scanタブの内容を確認（スキャン結果 or "Loading scan results..."）
    let contents = h.screen_contents();
    assert!(
        contents.contains("Scan Summary")
            || contents.contains("Loading scan results")
            || contents.contains("No scan results"),
        "Should show scan tab content, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecr_detail_shows_lifecycle_tab_when_switched() {
    let mut h = spawn_app(120, 24);
    h.select_service(1);
    h.wait_for_text("myapp/web").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("sha256:").unwrap();
    // ] を2回でLifecycleタブに切り替え
    h.send_char(']').unwrap();
    std::thread::sleep(Duration::from_millis(200));
    h.send_char(']').unwrap();
    std::thread::sleep(Duration::from_millis(500));
    // Lifecycleタブの内容を確認
    let contents = h.screen_contents();
    assert!(
        contents.contains("Lifecycle Policy") || contents.contains("Loading lifecycle"),
        "Should show lifecycle tab content, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ecr_detail_tab_cycles_back_when_bracket_keys() {
    let mut h = spawn_app(120, 24);
    h.select_service(1);
    h.wait_for_text("myapp/web").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("sha256:").unwrap();
    // ] で3回循環（Images → Scan → Lifecycle → Images）
    h.send_char(']').unwrap();
    std::thread::sleep(Duration::from_millis(200));
    h.send_char(']').unwrap();
    std::thread::sleep(Duration::from_millis(200));
    h.send_char(']').unwrap();
    std::thread::sleep(Duration::from_millis(500));
    // Imagesタブに戻っていることを確認
    h.wait_for_text("sha256:")
        .expect("Should cycle back to Images tab showing digests");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
