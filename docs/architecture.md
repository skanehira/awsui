# awsui アーキテクチャ設計書

## 1. システム概要

awsui は AWS SSO 経由で AWS リソースを管理するための TUI（Terminal User Interface）アプリケーションである。
Rust で実装され、ratatui + crossterm + tokio をベースとした非同期イベント駆動型アーキテクチャを採用している。

### 対応サービス

| サービス        | 操作                                                                                                |
| --------------- | --------------------------------------------------------------------------------------------------- |
| EC2             | 一覧・詳細表示、Start/Stop/Reboot/Terminate、SSM Connect                                            |
| ECR             | リポジトリ一覧、イメージ一覧                                                                        |
| ECS             | クラスター/サービス/タスク一覧・詳細、CloudWatch Logs 表示、Execute Command                         |
| S3              | バケット一覧、オブジェクト一覧（プレフィックスナビゲーション）、バケット作成/削除、オブジェクト削除 |
| VPC             | VPC 一覧、サブネット詳細                                                                            |
| Secrets Manager | シークレット一覧・詳細、作成/編集/削除                                                              |

### 技術スタック

| カテゴリ               | ライブラリ                            | バージョン                  |
| ---------------------- | ------------------------------------- | --------------------------- |
| 言語                   | Rust 2024 Edition                     | 1.92 (rust-toolchain.toml)  |
| TUI フレームワーク     | ratatui                               | 0.29                        |
| ターミナルバックエンド | crossterm                             | 0.28 (event-stream)         |
| 非同期ランタイム       | tokio                                 | 1 (full features)           |
| AWS SDK                | aws-sdk-*                             | 1 (behavior-version-latest) |
| テキスト入力           | tui-input                             | 0.11.1                      |
| ファジー検索           | nucleo                                | 0.5                         |
| エラーハンドリング     | thiserror                             | 2                           |
| CLI パーサー           | clap                                  | 4 (derive)                  |
| テスト                 | mockall 0.13, insta 1.46, rstest 0.26 |                             |

---

## 2. レイヤードアーキテクチャ

```
┌───────────────────────────────────────────────────────────┐
│                    main.rs (エントリポイント)             │
│  CLI パース → SSO 認証 → ターミナル初期化 → メインループ  │
├───────────────────────────────────────────────────────────┤
│                     TUI レイヤー (tui/)                   │
│  ┌──────────┐  ┌─────────────┐  ┌──────────────────────┐  │
│  │ input.rs │  │ components/ │  │      views/          │  │
│  │ キー入力 │  │ 再利用      │  │ サービス固有画面     │  │
│  │ → Action │  │ ウィジェット│  │                      │  │
│  └──────────┘  └─────────────┘  └──────────────────────┘  │
├───────────────────────────────────────────────────────────┤
│                 アプリケーションレイヤー                  │
│  ┌──────────┐  ┌─────────┐  ┌────────────┐  ┌──────────┐  │
│  │ app/     │  │ tab.rs  │  │action.rs   │  │ event.rs │  │
│  │ 状態管理 │  │タブ状態 │  │UIアクション│  │非同期    │  │
│  │dispatch  │  │データ   │  │            │  │イベント  │  │
│  └──────────┘  └─────────┘  └────────────┘  └──────────┘  │
├───────────────────────────────────────────────────────────┤
│                  AWS レイヤー (aws/)                      │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────┐    │
│  │ client traits│  │ model structs │  │ mock_clients │    │
│  │ 非同期 API   │  │ ドメインモデル│  │ テスト用     │    │
│  └──────────────┘  └───────────────┘  └──────────────┘    │
├───────────────────────────────────────────────────────────┤
│                 インフラストラクチャ                      │
│  ┌───────────┐  ┌─────────┐  ┌────────┐  ┌────────────┐   │
│  │config.rs  │  │ sso.rs  │  │ cli.rs │  │ fuzzy.rs   │   │
│  │AWSプロファ│  │SSOトーク│  │CLI引数 │  │ファジー検索│   │
│  │イル解析   │  │ン検証   │  │パース  │  │            │   │
│  └───────────┘  └─────── ─┘  └────────┘  └────────────┘   │
└───────────────────────────────────────────────────────────┘
```

### レイヤー間の依存関係

- **main.rs**: 全レイヤーをオーケストレーション。tokio::select! による並行イベント処理
- **TUI レイヤー**: アプリケーションレイヤーの状態を参照（読み取り専用）し描画。キー入力を Action に変換
- **アプリケーションレイヤー**: Action を受けて状態更新、SideEffect を返す。AppEvent で非同期結果を受信
- **AWS レイヤー**: trait ベースの非同期クライアント。アプリケーションレイヤーから Arc<dyn Trait> で参照
- **インフラストラクチャ**: 設定ファイル解析、SSO 認証、CLI パースなどの基盤機能

---

## 3. イベント駆動アーキテクチャ

### メインループ（tokio::select!）

```
┌─────────────────────────────────────────────────────────────┐
│                   tokio::select! ループ                     │
│                                                             │
│  ┌─ Branch 1: AppEvent受信 ────────────────────────────┐    │
│  │  app.event_rx.recv()                                │    │
│  │  → app.handle_event(event)                          │    │
│  │  → 必要に応じてリフレッシュ/ログポーリング管理      │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  ┌─ Branch 2: キー入力受信 ────────────────────────────┐    │
│  │  event_stream.next()                                │    │
│  │  → handle_key(&app, key) → Action                   │    │
│  │  → app.dispatch(action) → SideEffect                │    │
│  │  → handle_side_effects() → tokio::spawn (AWS呼出)   │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  ┌─ Branch 3: レンダリングタイマー ────────────────────┐    │
│  │  render_interval.tick() (16ms = 60FPS)              │    │
│  │  → terminal.draw(|frame| render(frame, &app))       │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### データフロー

```
KeyEvent (crossterm)
    │
    ▼
handle_key(&app, key) ─── コンテキスト認識（Mode/View ベース）
    │
    ▼
Action (enum)
    │
    ▼
app.dispatch(action) ─── 純粋な状態更新（副作用なし）
    │
    ├──▶ SideEffect::None ─── 状態更新のみ
    │
    ├──▶ SideEffect::Confirm(action) ─── EC2 Start/Stop/Reboot
    │        │
    │        ▼
    │    tokio::spawn → client.start_instances()
    │        │
    │        ▼
    │    tx.send(AppEvent::TabEvent(tab_id, ActionCompleted))
    │
    ├──▶ SideEffect::FormSubmit(ctx) ─── S3/Secrets CRUD
    │
    ├──▶ SideEffect::DangerAction(action) ─── Terminate/Delete
    │
    ├──▶ SideEffect::StartSsoLogin ─── SSO ログインプロセス起動
    │
    ├──▶ SideEffect::SsmConnect ─── TUI一時停止 → aws ssm
    │
    └──▶ SideEffect::EcsExec ─── TUI一時停止 → aws ecs execute-command
```

### 非同期通信チャネル

```
                   mpsc::channel(32)
tokio::spawn ─── event_tx ──────────▶ event_rx ─── app.handle_event()
(AWS API呼出)                                      (状態更新)
```

---

## 4. マルチタブアーキテクチャ

### タブの構造

```
App
 ├── tabs: Vec<Tab>           ← 開いているタブ一覧
 ├── active_tab_index: usize  ← アクティブタブ
 └── next_tab_id: u32         ← ID生成カウンタ

Tab (自己完結した単位)
 ├── id: TabId(u32)           ← 一意識別子
 ├── service: ServiceKind     ← EC2/ECR/ECS/S3/VPC/Secrets
 ├── tab_view: TabView        ← List / Detail
 ├── mode: Mode               ← Normal/Filter/Confirm/Form/...
 ├── loading: bool            ← ローディング状態
 ├── selected_index: usize    ← リスト選択位置
 ├── filter_input: Input      ← フィルタテキスト
 ├── data: ServiceData        ← サービス固有データ
 └── navigation_stack: Vec<NavigationEntry>  ← パンくず
```

### タブのライフサイクル

```
Dashboard ──(Enter/Ctrl+t)──▶ Tab作成(List, loading=true)
                                    │
                     create_client_and_load()
                                    │
                                    ▼
                           AWS API呼出(spawn)
                                    │
                                    ▼
                    TabEvent::*Loaded → loading=false
                                    │
         ┌──────────────────────────┼──────────────────┐
         │                          │                  │
    ユーザー操作              Enter(Detail)         Esc(Back)
    (フィルタ/選択/           loading=true             │
     コピー/リフレッシュ)           │                  │
         │                    詳細データ取得        List復帰
         ▼                          │
    状態更新                        ▼
                                Detail表示
```

### ServiceData（判別共用体）

各タブはサービス固有のデータを `ServiceData` enum で保持する。

```
ServiceData
 ├── Ec2 { instances: FilterableList<Instance> }
 ├── Ecr { repositories: FilterableList<Repository>, images: Vec<Image> }
 ├── Ecs { clusters: FilterableList<Cluster>, services, tasks, nav_level }
 ├── S3  { buckets: FilterableList<Bucket>, objects, selected_bucket, current_prefix }
 ├── Vpc { vpcs: FilterableList<Vpc>, subnets: Vec<Subnet> }
 └── Secrets { secrets: FilterableList<Secret>, detail, detail_tab, value_visible }
```

---

## 5. モード状態マシン

アプリケーションの UI モードは `Mode` enum で表現される。各タブが独立した Mode を持つ。

```
                    ┌──────────┐
         ┌──────────│  Normal  │──────────┐
         │          └──────────┘          │
         │            │  │  │             │
       / (フィルタ)   S  R  D           c (作成)
         │            │  │  │             │
         ▼            ▼  ▼  ▼             ▼
    ┌──────────┐  ┌──────────┐    ┌──────────────┐
    │  Filter  │  │ Confirm  │    │     Form     │
    └──────────┘  └──────────┘    └──────────────┘
    Enter/Esc→Normal  y/n→Normal  Enter(送信)/Esc→Normal

                    ┌──────────┐
                    │  Normal  │
                    └──────────┘
                      │     │
                    D (削除)  ? (ヘルプ)
                      │     │
                      ▼     ▼
              ┌──────────────┐  ┌──────┐
              │DangerConfirm │  │ Help │
              └──────────────┘  └──────┘
              入力一致→実行      Esc→Normal
              Esc→Normal
```

### キー入力の優先順位

```
1. グローバルオーバーレイ（最高優先度）
   ├── Message ダイアログ（app.message）
   ├── Help ポップアップ（app.show_help）
   ├── SSO Login ダイアログ
   ├── Profile Selector
   └── Service Picker

2. ダッシュボード
   └── サービス選択/フィルタ

3. タブ固有モーダル
   ├── Confirm（y/n）
   ├── Form（Enter/Esc/Tab）
   ├── DangerConfirm（テキスト入力+Enter）
   └── ContainerSelect（j/k/Enter/Esc）

4. タブ操作（Normal モードのみ）
   ├── Tab/Shift+Tab（タブ切替）
   ├── Ctrl+w（タブ閉じ）
   └── Ctrl+t（サービスピッカー）

5. ビュー固有ハンドラ（最低優先度）
   └── サービス×ビュー（List/Detail）ごとのキーマッピング
```

---

## 6. AWS クライアント設計

### トレイトベースの抽象化

```rust
#[cfg_attr(test, automock)]
#[async_trait]
pub trait Ec2Client: Send + Sync {
    async fn describe_instances(&self) -> Result<Vec<Instance>, AppError>;
    async fn start_instances(&self, ids: &[String]) -> Result<(), AppError>;
    async fn stop_instances(&self, ids: &[String]) -> Result<(), AppError>;
    async fn reboot_instances(&self, ids: &[String]) -> Result<(), AppError>;
    async fn terminate_instances(&self, ids: &[String]) -> Result<(), AppError>;
}
```

### クライアント管理

```
main.rs: Clients構造体
 ├── ec2:     Option<Arc<dyn Ec2Client>>
 ├── ecr:     Option<Arc<dyn EcrClient>>
 ├── ecs:     Option<Arc<dyn EcsClient>>
 ├── s3:      Option<Arc<dyn S3Client>>
 ├── vpc:     Option<Arc<dyn VpcClient>>
 ├── secrets: Option<Arc<dyn SecretsClient>>
 └── logs:    Option<Arc<dyn LogsClient>>
```

- クライアントはタブ作成時にオンデマンドで生成される
- ダッシュボードに戻ると全クライアントをクリア
- `Arc<dyn Trait>` で tokio::spawn 内から安全にアクセス

### テスト戦略

- **mockall**: `#[cfg_attr(test, automock)]` により自動生成されるモック
- **mock-data feature**: `feature = "mock-data"` でハードコードされたサンプルデータを使用するクライアント実装

---

## 7. 起動フロー

```
main()
 │
 ├── 1. CLI引数パース (clap)
 │      --allow-delete [SERVICES]
 │      --profile NAME
 │
 ├── 2. プロファイル読み込み
 │      ~/.aws/config → SsoProfile[]
 │
 ├── 3. SSO認証
 │      ├── --profile指定あり → 同期的にSSOトークンチェック
 │      │     └── 期限切れ → aws sso login (ブロッキング)
 │      └── --profile指定なし → プロファイル選択画面
 │            └── 選択後 → 非同期SSO login (バックグラウンド)
 │
 ├── 4. App初期化
 │      ├── with_delete_permissions() ← --profile指定時
 │      └── new_with_profile_selector() ← 対話選択時
 │
 ├── 5. ターミナル初期化
 │      enable_raw_mode + EnterAlternateScreen
 │
 ├── 6. メインループ (tokio::select!)
 │
 └── 7. ターミナル復元
        disable_raw_mode + LeaveAlternateScreen
```

---

## 8. モジュール構成

```
src/
├── main.rs              エントリポイント、メインループ、レンダリング、副作用処理
├── lib.rs               ライブラリルート（モジュール公開）
│
├── app/                 アプリケーション状態
│   ├── mod.rs           App構造体、dispatch()、handle_event()
│   ├── crud.rs          CRUD操作ハンドラ
│   └── tests.rs         Appのユニットテスト
│
├── tab.rs               Tab構造体、FilterableList<T>、ServiceData、EcsNavLevel
├── action.rs            Action enum（UIアクション）
├── event.rs             AppEvent/TabEvent enum（非同期イベント）
├── ui_state.rs          Mode、SideEffect、各種コンテキスト型
├── service.rs           ServiceKind enum
├── error.rs             AppError (thiserror)
├── cli.rs               CLI引数パース、DeletePermissions
├── config.rs            ~/.aws/config パース、SsoProfile
├── sso.rs               SSOトークンキャッシュ検証
├── fuzzy.rs             nucleo によるファジーフィルタリング
├── recent.rs            最近使用したサービスの追跡
│
├── aws/                 AWS SDK統合
│   ├── client.rs        Ec2Client trait + AwsEc2Client
│   ├── ecr_client.rs    EcrClient trait + AwsEcrClient
│   ├── ecs_client.rs    EcsClient trait + AwsEcsClient
│   ├── s3_client.rs     S3Client trait + AwsS3Client
│   ├── vpc_client.rs    VpcClient trait + AwsVpcClient
│   ├── secrets_client.rs SecretsClient trait + AwsSecretsClient
│   ├── logs_client.rs   LogsClient trait + AwsLogsClient
│   ├── model.rs         EC2ドメインモデル (Instance, InstanceState, Volume)
│   ├── ecr_model.rs     ECRモデル (Repository, Image)
│   ├── ecs_model.rs     ECSモデル (Cluster, Service, Task, ContainerLogConfig)
│   ├── s3_model.rs      S3モデル (Bucket, S3Object)
│   ├── vpc_model.rs     VPCモデル (Vpc, Subnet)
│   ├── secrets_model.rs Secretsモデル (Secret, SecretDetail)
│   ├── logs_model.rs    Logsモデル (LogEvent)
│   ├── mock_clients.rs  mock-data feature用クライアント
│   └── mock_data.rs     サンプルデータ
│
└── tui/                 UIレイヤー
    ├── input.rs         KeyEvent → Action マッピング
    ├── theme.rs         カラーパレット、スタイル関数
    ├── components/      再利用可能ウィジェット
    │   ├── table.rs         SelectableTable
    │   ├── tab_bar.rs       TabBar（タブバー）
    │   ├── status_bar.rs    StatusBar（フッター）
    │   ├── dialog.rs        ConfirmDialog / MessageDialog
    │   ├── form_dialog.rs   FormDialog（フォーム入力）
    │   ├── danger_confirm.rs DangerConfirmDialog（危険操作確認）
    │   ├── help.rs          HelpPopup（ヘルプ）
    │   ├── loading.rs       Loading（スピナー）
    │   ├── list_selector.rs ListSelector
    │   └── service_picker.rs ServicePicker
    └── views/           画面ビュー
        ├── profile_select.rs  プロファイル選択
        ├── dashboard.rs       ダッシュボード
        ├── service_select.rs  サービス選択
        ├── ec2_list.rs / ec2_detail.rs
        ├── ecr_list.rs / ecr_detail.rs
        ├── ecs_list.rs / ecs_detail.rs / ecs_service_detail.rs / ecs_task_detail.rs / ecs_log.rs
        ├── s3_list.rs / s3_detail.rs
        ├── vpc_list.rs / vpc_detail.rs
        └── secrets_list.rs / secrets_detail.rs
```

---

## 9. 画面遷移図

```
                    ┌───────────────────┐
                    │ Profile Select    │
                    │ (--profile省略時) │
                    └────────┬──────────┘
                             │ Enter (SSO Login)
                             ▼
                    ┌──────────────────┐
       ┌──────────  │   Dashboard      │ ──────────┐
       │            │ Recently Used    │           │
       │            │ All Services     │           │
       │            └──────────────────┘           │
       │ Enter                           Ctrl+t    │
       ▼                                           ▼
┌──────────────┐                        ┌──────────────┐
│ Tab (List)   │◀───── 新規タブ ────────│ServicePicker │
│ EC2/ECR/...  │                        └──────────────┘
└──────────────────────────────────────────────────────┘
       │ Enter
       ▼
┌────────────────────────────────────────────────────┐
│ Tab (Detail) │─────────────────────▶│ Tab (Detail) │
│ EC2 Instance │  (EC2→VPC遷移)       │ VPC Subnet   │
└────────────────────────────────────────────────────┘
       │
       │ (ECS の場合、4段階ナビゲーション)
       ▼
   Cluster Detail → Service Detail → Task Detail → Log View
```

---

## 10. 設計上の判断

### dispatch() の純粋性

`app.dispatch(action)` は副作用を持たず、`SideEffect` enum を返す。実際の AWS API 呼出やプロセス起動は main.rs の `handle_side_effects()` が担当する。これにより、状態遷移ロジックがテスト可能になっている。

### FilterableList<T> によるフィルタリング

全アイテム（`items`）とフィルタ済みアイテム（`filtered`）を分離することで、フィルタのリセットを O(n) の clone で実現している。ファジー検索には nucleo を使用し、名前フィールドにボーナスを付与することで直感的なフィルタリングを提供する。

### EcsNavLevel による複雑なナビゲーション

ECS は Cluster → Service → Task → Log View の4段階のネストを持つ。これを `EcsNavLevel` enum で表現し、Back 操作時に段階的に戻る。LogViewState は大きいため `Box<LogViewState>` でヒープに配置している。

### TUI 一時停止/復帰

SSM Connect と ECS Execute Command は対話的なシェルセッションを必要とするため、TUI を一時停止（LeaveAlternateScreen + disable_raw_mode）し、コマンド終了後に復帰する。

### 削除権限の制御

`--allow-delete` フラグにより、安全性を確保しつつ削除操作を制御する。デフォルトでは全サービスの削除が禁止されており、明示的な許可が必要である。
