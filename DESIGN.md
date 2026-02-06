# DESIGN.md - AWS Dashboard TUI (MVP)

## 概要

AWS SSOで認証し、EC2インスタンスの閲覧・操作ができるTUIツール。

## MVPスコープ

| 項目       | 内容                                          |
| ---------- | --------------------------------------------- |
| 認証       | AWS SSO（起動時にプロファイル選択）           |
| リージョン | `~/.aws/config` のプロファイル設定値を使用    |
| 対象       | EC2 Instances のみ                            |
| 読み取り   | 一覧表示、詳細表示（Overview/Tags）、フィルタ |
| 操作       | Start, Stop, Reboot（確認ダイアログ付き）     |

## 画面遷移

```
起動
 │
 ▼
┌──────────────────┐
│ Profile選択画面  │
└────────┬─────────┘
         │ Enter
         ▼
┌─────────────────────────────────────────────────┐
│ EC2インスタンス   │─────────→│ インスタンス詳細 │
│ 一覧画面          │←─────────│                  │
└─────────────────────────────────────────────────┘
        │ S/R
        ▼
┌──────────────────┐
│ 確認ダイアログ   │
└──────────────────┘

※ メッセージダイアログはどの画面からでも表示される（エラー、空結果、操作結果等）
※ ヘルプポップアップは ? キーでどの画面からでも表示可能
```

## 画面仕様

### 画面1: SSOプロファイル選択

```
┌──────────────────────────────────────────────────────────────────┐
│                                                                  │
│                                                                  │
│                   Select AWS SSO Profile                         │
│                                                                  │
│                   ▶ dev-account                                  │
│                     staging-account                              │
│                     prod-account                                 │
│                     sandbox                                      │
│                                                                  │
│                                                                  │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│ j/k:select  Enter:confirm  q:quit                                │
└──────────────────────────────────────────────────────────────────┘
```

- `~/.aws/config` から `[profile xxx]` のうち `sso_start_url` を持つプロファイルを一覧表示
- 選択したプロファイルの `region` 設定をアプリ全体で使用

### 画面2: EC2インスタンス一覧

```
┌─ EC2 Instances ─────────────────────────────────────────────────┐
│ Filter: _______________________________________________  /      │
├─────────────────────────────────────────────────────────────────┤
│ Instance ID  │ Name     │ State     │ Type     │ AZ             │
├─────────────────────────────────────────────────────────────────┤
│▶i-0abc1234   │ web-01   │ ● running │ t3.micro │ ap-ne-1a       │
│ i-0def5678   │ api-01   │ ○ stopped │ t3.small │ ap-ne-1c       │
│ i-0ghi9012   │ batch-01 │ ● running │ m5.large │ ap-ne-1a       │
│ i-0jkl3456   │ worker-01│ ◐ pending │ t3.nano  │ ap-ne-1d       │
│ i-0mno7890   │ db-01    │ ● running │ r5.xlarge│ ap-ne-1a       │
│              │          │           │          │                │
│              │          │           │          │                │
│              │          │           │          │                │
├─────────────────────────────────────────────────────────────────┤
│ NORMAL │ dev-account │ ap-northeast-1 │ 1-5/5                   │
│ j/k:move Enter:detail S:start/stop R:reboot /:filter ?:help     │
└─────────────────────────────────────────────────────────────────┘
```

ステータスアイコン:
- `●` running (Green)
- `○` stopped (Red)
- `◐` pending/stopping/shutting-down (Yellow)
- `◌` terminated (DarkGray)

### 画面3: インスタンス詳細

```
┌─────────────────────────────────────────────────────────────────┐
│ [Overview] [Tags]                                               │
├─────────────────────────────────────────────────────────────────┤
│ ┌─ Instance ────────────────────┬─ Network ──────────────────┐  │
│ │ ID:      i-0abc1234           │ VPC:       vpc-abc123      │  │
│ │ Name:    web-01               │ Subnet:    subnet-def456   │  │
│ │ Type:    t3.micro             │ Private IP:10.0.1.42       │  │
│ │ State:   ● running            │ Public IP: 54.210.3.15     │  │
│ │ AMI:     ami-0abcdef123       │ AZ:        ap-ne-1a        │  │
│ │ Key:     my-keypair           │ SG:        sg-789012       │  │
│ │ Platform:Linux/UNIX           │                            │  │
│ │ Launch:  2025-01-10 09:30     │                            │  │
│ └───────────────────────────────┴────────────────────────────┘  │
│ ┌─ Storage ──────────────────────────────────────────────────┐  │
│ │ vol-abc123  gp3  20GB  /dev/xvda  attached                 │  │
│ │ vol-def456  gp3  100GB /dev/xvdf  attached                 │  │
│ └────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│ NORMAL │ dev-account │ ap-northeast-1                           │
│ Tab:switch-tab S:start/stop R:reboot y:copy-id Esc:back         │
└─────────────────────────────────────────────────────────────────┘
```

### 画面3b: 詳細 - Tagsタブ

```
┌─────────────────────────────────────────────────────────────────┐
│ [Overview] [Tags]                                               │
├──────────────────────────────┬──────────────────────────────────┤
│ Key                          │ Value                            │
├──────────────────────────────┼──────────────────────────────────┤
│ Name                         │ web-01                           │
│ env                          │ production                       │
│ team                         │ backend                          │
│ service                      │ web-api                          │
│                              │                                  │
├──────────────────────────────┴──────────────────────────────────┤
│ NORMAL │ dev-account │ ap-northeast-1                           │
│ Tab:switch-tab y:copy-value Esc:back                            │
└─────────────────────────────────────────────────────────────────┘
```

### 確認ダイアログ

```
         ┌─ Confirm ───────────────────────────────────┐
         │                                             │
         │  Stop instance i-0abc1234 (web-01)?         │
         │                                             │
         │          [ Yes (y) ]    [ No (n) ]          │
         │                                             │
         └─────────────────────────────────────────────┘
```

### メッセージダイアログ

エラー、空結果、操作完了など様々な通知に使用する汎用ダイアログ。

```
         ┌─ Info ──────────────────────────────────────┐
         │                                             │
         │  No instances found in ap-northeast-1       │
         │                                             │
         │              [ OK (Enter) ]                 │
         │                                             │
         └─────────────────────────────────────────────┘
```

```
         ┌─ Error ─────────────────────────────────────┐
         │                                             │
         │  Failed to describe instances               │
         │  AccessDeniedException: User is not         │
         │  authorized to perform ec2:DescribeInstances│
         │                                             │
         │              [ OK (Enter) ]                 │
         │                                             │
         └─────────────────────────────────────────────┘
```

```
         ┌─ Success ───────────────────────────────────┐
         │                                             │
         │  Instance i-0abc1234 stop initiated         │
         │                                             │
         │              [ OK (Enter) ]                 │
         │                                             │
         └─────────────────────────────────────────────┘
```

### ヘルプポップアップ

```
         ┌─ Help ───────────────────────────────────────────────┐
         │                                                      │
         │  Navigation                                          │
         │    j/k        Move down/up                           │
         │    g/G        Go to first/last                       │
         │    Ctrl+d/u   Half page down/up                      │
         │    Enter      Open detail                            │
         │    Esc        Go back                                │
         │                                                      │
         │  Actions                                             │
         │    S          Start/Stop instance                    │
         │    R          Reboot instance                        │
         │    r          Refresh list                           │
         │    y          Copy instance ID                       │
         │    /          Filter instances                       │
         │                                                      │
         │  General                                             │
         │    ?          Show this help                         │
         │    q          Quit                                   │
         │                                                      │
         │  Press Esc to close                                  │
         └──────────────────────────────────────────────────────┘
```

### ローディング状態

```
┌──────────────────────────────────────────────────────────────────┐
│                                                                  │
│                                                                  │
│                  ⠋ Loading instances...                          │
│                                                                  │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│ NORMAL │ dev-account │ ap-northeast-1 │ Loading...               │
└──────────────────────────────────────────────────────────────────┘
```

## キーバインド

### グローバル（全画面共通）

| キー           | 機能                      |
| -------------- | ------------------------- |
| `q` / `Ctrl+c` | 終了                      |
| `?`            | ヘルプ表示                |
| `Esc`          | 戻る / ダイアログを閉じる |

### Profile選択画面

| キー    | 機能             |
| ------- | ---------------- |
| `j/k`   | 上下移動         |
| `Enter` | プロファイル確定 |

### EC2一覧画面 (Normalモード)

| キー       | 機能                         |
| ---------- | ---------------------------- |
| `j/k`      | カーソル上下移動             |
| `g/G`      | 先頭/末尾                    |
| `Ctrl+d/u` | 半ページスクロール           |
| `Enter`    | 詳細画面を開く               |
| `/`        | フィルタモード開始           |
| `S`        | Start/Stop（確認ダイアログ） |
| `R`        | Reboot（確認ダイアログ）     |
| `r`        | リフレッシュ                 |
| `y`        | インスタンスIDをコピー       |

### EC2一覧画面 (Filterモード)

| キー     | 機能                               |
| -------- | ---------------------------------- |
| 文字入力 | インクリメンタルフィルタ           |
| `Enter`  | フィルタ確定、Normalモードに戻る   |
| `Esc`    | フィルタクリア、Normalモードに戻る |
| `Ctrl+w` | 1単語削除                          |
| `Ctrl+u` | 行クリア                           |

### 詳細画面

| キー  | 機能                   |
| ----- | ---------------------- |
| `Tab` | Overview/Tags タブ切替 |
| `j/k` | スクロール（Tags時）   |
| `S`   | Start/Stop             |
| `R`   | Reboot                 |
| `y`   | インスタンスIDをコピー |
| `Esc` | 一覧に戻る             |

### 確認ダイアログ

| キー        | 機能       |
| ----------- | ---------- |
| `y`         | 実行       |
| `n` / `Esc` | キャンセル |

### メッセージダイアログ

| キー            | 機能   |
| --------------- | ------ |
| `Enter` / `Esc` | 閉じる |

## アーキテクチャ

### パターン: イベント駆動 + コンポーネント指向

```
KeyEvent
    │
    ▼
handle_key() ──→ Action
    │
    ▼
app.dispatch(action)
    │
    ├──→ 状態更新（同期）
    │
    └──→ AWS API呼び出し（tokio::spawn）
              │
              ▼
         mpsc::channel ──→ AppEvent
              │
              ▼
         app.handle_event() ──→ 状態更新
              │
              ▼
         terminal.draw(|f| render(f, &app))
```

### モジュール構成

```
src/
├── main.rs                     # エントリポイント、ターミナル初期化、メインループ
├── app.rs                      # App状態管理
├── event.rs                    # AppEvent enum
├── action.rs                   # Action enum
├── error.rs                    # AppError (thiserror)
├── config.rs                   # SSOプロファイル読み込み (~/.aws/config)
│
├── aws/                        # AWS SDK統合層
│   ├── mod.rs
│   ├── client.rs               # Ec2Client trait + 実装
│   └── model.rs                # Instance, InstanceState 等のドメインモデル
│
└── tui/                        # UI層
    ├── mod.rs
    ├── input.rs                # キー入力ハンドラ
    ├── theme.rs                # カラーパレット
    │
    ├── components/             # 再利用可能ウィジェット
    │   ├── mod.rs
    │   ├── table.rs            # 選択可能テーブル
    │   ├── dialog.rs           # 確認ダイアログ + メッセージダイアログ
    │   ├── help.rs             # ヘルプポップアップ
    │   ├── loading.rs          # ローディングスピナー
    │   ├── status_bar.rs       # ステータスバー
    │   └── list_selector.rs    # リスト選択（Profile選択用）
    │
    └── views/                  # 画面ビュー
        ├── mod.rs
        ├── profile_select.rs   # Profile選択画面
        ├── ec2_list.rs         # EC2インスタンス一覧
        └── ec2_detail.rs       # EC2インスタンス詳細
```

### 主要な型定義

```rust
// app.rs
pub struct App {
    pub mode: Mode,
    pub view: View,
    pub profile: Option<String>,
    pub region: Option<String>,
    pub should_quit: bool,
    pub loading: bool,
    pub message: Option<Message>,

    // EC2 state
    pub instances: Vec<Instance>,
    pub selected_index: usize,
    pub filter_text: String,
    pub detail_tab: DetailTab,

    // async communication
    event_tx: mpsc::Sender<AppEvent>,
    event_rx: mpsc::Receiver<AppEvent>,
}

pub enum Mode {
    Normal,
    Filter,
    Confirm(ConfirmAction),
    Message,
}

pub enum View {
    ProfileSelect,
    Ec2List,
    Ec2Detail,
}

pub enum DetailTab {
    Overview,
    Tags,
}

pub struct Message {
    pub level: MessageLevel,
    pub title: String,
    pub body: String,
}

pub enum MessageLevel {
    Info,
    Success,
    Error,
}

// action.rs
pub enum Action {
    Quit,
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    HalfPageUp,
    HalfPageDown,
    Enter,
    Back,
    Refresh,
    CopyId,
    StartFilter,
    ConfirmFilter,
    CancelFilter,
    FilterInput(char),
    FilterDeleteWord,
    FilterClearLine,
    StartStop,
    Reboot,
    ConfirmYes,
    ConfirmNo,
    DismissMessage,
    ShowHelp,
    SwitchDetailTab,
    Noop,
}

// event.rs
pub enum AppEvent {
    InstancesLoaded(Result<Vec<Instance>, AppError>),
    ActionCompleted(Result<String, AppError>),
}

// error.rs
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("AWS API error: {0}")]
    AwsApi(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### AWS Client trait

```rust
// aws/client.rs
#[cfg_attr(test, automock)]
#[async_trait]
pub trait Ec2Client: Send + Sync {
    async fn describe_instances(&self) -> Result<Vec<Instance>, AppError>;
    async fn start_instances(&self, ids: &[String]) -> Result<(), AppError>;
    async fn stop_instances(&self, ids: &[String]) -> Result<(), AppError>;
    async fn reboot_instances(&self, ids: &[String]) -> Result<(), AppError>;
}
```

## カラーパレット

16色ベース（ターミナル互換性重視）:

| 要素                          | 前景色            | 修飾 |
| ----------------------------- | ----------------- | ---- |
| アクティブタブ / フォーカス枠 | Cyan              | Bold |
| 非アクティブタブ              | DarkGray          | -    |
| 選択行                        | White on DarkGray | Bold |
| テーブルヘッダー              | Yellow            | Bold |
| ステータスバー                | White on DarkBlue | -    |
| State: running                | Green             | -    |
| State: stopped                | Red               | -    |
| State: pending系              | Yellow            | -    |
| State: terminated             | DarkGray          | -    |
| メッセージ: Error             | Red               | Bold |
| メッセージ: Success           | Green             | -    |
| メッセージ: Info              | Cyan              | -    |

## テスト戦略

### TDD原則
- RED → GREEN → REFACTOR サイクルを厳守
- テストファーストで実装

### テスト種別

| 種別                   | 対象         | ツール                     |
| ---------------------- | ------------ | -------------------------- |
| ユニットテスト         | 各モジュール | rstest, mockall            |
| スナップショットテスト | UI描画       | insta, ratatui TestBackend |
| 非同期テスト           | AWS API層    | tokio::test                |

### テスト命名規則
```
関数_returns結果_when条件
```

### AWS APIのモック
- `Ec2Client` traitに対して `mockall` でモック生成
- テスト時は `MockEc2Client` を注入

## 依存クレート

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
aws-config = { version = "1", features = ["behavior-version-latest"] }
aws-sdk-ec2 = "1"
clap = { version = "4", features = ["derive"] }
thiserror = "2"
anyhow = "1"
async-trait = "0.1"
futures = "0.3"
cli-clipboard = "0.4"

[dev-dependencies]
insta = "1.46"
rstest = "0.26"
mockall = "0.13"
pretty_assertions = "1"
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread"] }
```
