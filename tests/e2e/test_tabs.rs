use crate::common::spawn_app;

#[test]
fn new_tab_opens_when_ctrl_t() {
    let mut h = spawn_app(80, 24);
    h.wait_for_text("All Services").unwrap();
    // ダッシュボードからEC2を選択してタブを作成
    h.send_enter().unwrap();
    h.wait_for_text("EC2 Instances").unwrap();
    // Ctrl+t でサービスピッカーを開く
    h.send_ctrl('t').unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    // 下に移動してECRを選択
    h.send_down().unwrap();
    h.send_enter().unwrap();
    // 新しいタブが作成される
    h.wait_for_text("ECR").expect("Should create ECR tab");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn tab_switches_when_tab_pressed() {
    let mut h = spawn_app(120, 24);
    h.wait_for_text("All Services").unwrap();
    // EC2タブを作成
    h.send_enter().unwrap();
    h.wait_for_text("EC2 Instances").unwrap();
    // Ctrl+t で新しいタブ（ECR）を作成
    h.send_ctrl('t').unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    h.send_down().unwrap();
    h.send_enter().unwrap();
    // ECRのデータ読み込み完了を待つ
    std::thread::sleep(std::time::Duration::from_millis(500));
    // Tabで次のタブに切り替え（ECR→EC2に戻る）
    h.send_tab().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let contents = h.screen_contents();
    assert!(
        contents.contains("EC2 Instances"),
        "Should switch to EC2 tab, got: {}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn tab_closes_when_ctrl_w() {
    let mut h = spawn_app(80, 24);
    h.wait_for_text("All Services").unwrap();
    // EC2タブを作成
    h.send_enter().unwrap();
    h.wait_for_text("EC2 Instances").unwrap();
    // Ctrl+w でタブを閉じる → ダッシュボードに戻る
    h.send_ctrl('w').unwrap();
    h.wait_for_text("All Services")
        .expect("Should return to dashboard after closing last tab");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
