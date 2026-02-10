use crate::common::spawn_app;

#[test]
fn s3_list_shows_buckets_when_selected() {
    let mut h = spawn_app(120, 24);
    h.select_service(3); // S3 = index 3
    h.wait_for_text("my-app-assets-prod")
        .expect("S3 list should show mock buckets");
    let contents = h.screen_contents();
    assert!(
        contents.contains("S3 Buckets"),
        "Should show S3 Buckets header, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn s3_detail_shows_objects_when_entered() {
    let mut h = spawn_app(120, 24);
    h.select_service(3);
    h.wait_for_text("my-app-assets-prod").unwrap();
    // 最初のバケットを選択
    h.send_enter().unwrap();
    // オブジェクト一覧のロード完了を待つ
    h.wait_for_text("index.html")
        .expect("S3 detail should show objects");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn s3_back_returns_to_list_when_escape() {
    let mut h = spawn_app(120, 24);
    h.select_service(3);
    h.wait_for_text("my-app-assets-prod").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("index.html").unwrap();
    // Escでバケット一覧に戻る
    h.send_esc().unwrap();
    h.wait_for_text("S3 Buckets")
        .expect("Should return to S3 bucket list");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
