use std::time::Duration;

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

#[test]
fn s3_detail_shows_settings_tab_when_switched() {
    let mut h = spawn_app(120, 24);
    h.select_service(3);
    h.wait_for_text("my-app-assets-prod").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("index.html").unwrap();
    // ] でSettingsタブに切り替え
    h.send_char(']').unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let contents = h.screen_contents();
    assert!(
        contents.contains("Bucket Settings") || contents.contains("Region"),
        "Should show settings tab content, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn s3_detail_shows_preview_when_object_selected() {
    let mut h = spawn_app(120, 24);
    h.select_service(3);
    h.wait_for_text("my-app-assets-prod").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("index.html").unwrap();
    // index.html（2番目のアイテム、1つ目はimages/プレフィックス）を選択してEnter
    h.send_down().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    h.send_enter().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    // プレビューが表示される（ロード中またはコンテンツ）
    let contents = h.screen_contents();
    assert!(
        contents.contains("Preview:") || contents.contains("Hello, World!"),
        "Should show preview content, got:\n{}",
        contents
    );
    // Escでプレビューを閉じる
    h.send_esc().unwrap();
    h.wait_for_text("index.html")
        .expect("Should return to object list after closing preview");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn s3_detail_shows_download_form_when_d_pressed() {
    let mut h = spawn_app(120, 24);
    h.select_service(3);
    h.wait_for_text("my-app-assets-prod").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("index.html").unwrap();
    // ファイルオブジェクトを選択（index.htmlは2番目）
    h.send_down().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    // dキーでダウンロードフォームを表示
    h.send_char('d').unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let contents = h.screen_contents();
    assert!(
        contents.contains("Download Object") && contents.contains("Save Directory"),
        "Should show download form, got:\n{}",
        contents
    );
    // Escでフォームを閉じる
    h.send_esc().unwrap();
    h.wait_for_text("index.html")
        .expect("Should return to object list after cancelling download");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn s3_detail_shows_upload_form_when_u_pressed() {
    let mut h = spawn_app(120, 24);
    h.select_service(3);
    h.wait_for_text("my-app-assets-prod").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("index.html").unwrap();
    // uキーでアップロードフォームを表示
    h.send_char('u').unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let contents = h.screen_contents();
    assert!(
        contents.contains("Upload Object") && contents.contains("Local File Path"),
        "Should show upload form, got:\n{}",
        contents
    );
    // Escでフォームを閉じる
    h.send_esc().unwrap();
    h.wait_for_text("index.html")
        .expect("Should return to object list after cancelling upload");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
