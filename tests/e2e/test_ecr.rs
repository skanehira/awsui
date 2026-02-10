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
