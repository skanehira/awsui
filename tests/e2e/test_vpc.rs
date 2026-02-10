use crate::common::spawn_app;

#[test]
fn vpc_list_shows_vpcs_when_selected() {
    let mut h = spawn_app(120, 24);
    h.select_service(4); // VPC = index 4
    h.wait_for_text("main-vpc")
        .expect("VPC list should show mock VPCs");
    let contents = h.screen_contents();
    assert!(
        contents.contains("VPCs"),
        "Should show VPCs header, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn vpc_detail_shows_subnets_when_entered() {
    let mut h = spawn_app(120, 24);
    h.select_service(4);
    h.wait_for_text("main-vpc").unwrap();
    // 最初のVPCを選択して詳細へ
    h.send_enter().unwrap();
    h.wait_for_text("public-subnet-1a")
        .expect("VPC detail should show subnets");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn vpc_back_returns_to_list_when_escape() {
    let mut h = spawn_app(120, 24);
    h.select_service(4);
    h.wait_for_text("main-vpc").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("public-subnet-1a").unwrap();
    // Escで一覧に戻る
    h.send_esc().unwrap();
    h.wait_for_text("VPCs").expect("Should return to VPC list");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
