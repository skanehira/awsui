use crate::common::spawn_app;

#[test]
fn secrets_list_shows_secrets_when_selected() {
    let mut h = spawn_app(120, 24);
    h.select_service(5); // SecretsManager = index 5
    h.wait_for_text("prod/database/password")
        .expect("Secrets list should show mock secrets");
    let contents = h.screen_contents();
    assert!(
        contents.contains("Secrets"),
        "Should show Secrets header, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn secrets_detail_shows_info_when_entered() {
    let mut h = spawn_app(120, 24);
    h.select_service(5);
    h.wait_for_text("prod/database/password").unwrap();
    // 最初のシークレットを選択して詳細へ
    h.send_enter().unwrap();
    h.wait_for_text("Overview")
        .expect("Secrets detail should show Overview tab");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn secrets_back_returns_to_list_when_escape() {
    let mut h = spawn_app(120, 24);
    h.select_service(5);
    h.wait_for_text("prod/database/password").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("Overview").unwrap();
    // Escで一覧に戻る
    h.send_esc().unwrap();
    h.wait_for_text("Secrets")
        .expect("Should return to Secrets list");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
