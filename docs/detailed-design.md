# awsui 詳細設計書

## 1. データ型定義

### 1.1 Action（UIアクション）

キー入力から変換される UI アクション。`app.dispatch()` で処理される。

```
Action
 ├── ナビゲーション
 │    MoveUp, MoveDown, MoveToTop, MoveToBottom,
 │    HalfPageUp, HalfPageDown, Enter, Back, Refresh
 │
 ├── コピー
 │    CopyId
 │
 ├── フィルタ
 │    StartFilter, ConfirmFilter, CancelFilter,
 │    FilterHandleInput(InputRequest)
 │
 ├── EC2固有
 │    StartStop, Reboot
 │
 ├── 確認ダイアログ
 │    ConfirmYes, ConfirmNo
 │
 ├── モーダル
 │    DismissMessage, ShowHelp
 │
 ├── 詳細ビュータブ
 │    SwitchDetailTab, PrevDetailTab, RevealSecretValue, FollowLink
 │
 ├── CRUD
 │    Create, Delete, Edit
 │
 ├── フォーム
 │    FormSubmit, FormCancel, FormNextField,
 │    FormHandleInput(InputRequest)
 │
 ├── 危険操作確認
 │    DangerConfirmSubmit, DangerConfirmCancel,
 │    DangerConfirmHandleInput(InputRequest)
 │
 ├── タブ操作
 │    NextTab, PrevTab, CloseTab, NewTab
 │
 ├── サービスピッカー
 │    PickerConfirm, PickerCancel, PickerMoveUp, PickerMoveDown,
 │    PickerHandleInput(InputRequest)
 │
 ├── ログ操作
 │    ShowLogs, LogScrollUp, LogScrollDown,
 │    LogScrollToTop, LogScrollToBottom,
 │    LogToggleAutoScroll, LogSearchNext, LogSearchPrev
 │
 ├── コンテナ選択
 │    ContainerSelectUp, ContainerSelectDown,
 │    ContainerSelectConfirm, ContainerSelectCancel
 │
 ├── シェルアクセス
 │    SsmConnect, EcsExec
 │
 ├── SSO
 │    CancelSsoLogin
 │
 └── その他
      Quit, Noop
```

### 1.2 AppEvent / TabEvent（非同期イベント）

バックグラウンドタスクから UI スレッドへ送信されるイベント。

```
AppEvent
 ├── TabEvent(TabId, TabEvent)    ← タブ固有のデータ更新
 ├── CrudCompleted(TabId, Result) ← CRUD操作完了
 ├── SsoLoginOutput(String)       ← SSO loginの出力行
 └── SsoLoginCompleted(Result)    ← SSO login完了

TabEvent
 ├── データ読み込み完了
 │    InstancesLoaded, RepositoriesLoaded, ImagesLoaded,
 │    ClustersLoaded, EcsServicesLoaded, EcsTasksLoaded,
 │    BucketsLoaded, ObjectsLoaded, VpcsLoaded, SubnetsLoaded,
 │    SecretsLoaded, SecretDetailLoaded, SecretValueLoaded
 │
 ├── ナビゲーション
 │    NavigateVpcLoaded(Result<(Vec<Vpc>, Vec<Subnet>)>)
 │
 ├── ログ
 │    EcsLogConfigsLoaded, EcsLogEventsLoaded
 │
 └── アクション
      ActionCompleted(Result<String>)
```

### 1.3 SideEffect

`dispatch()` が返す副作用。main.rs で実際の処理が行われる。

```
SideEffect
 ├── None                                    ← 副作用なし（状態更新のみ）
 ├── Confirm(ConfirmAction)                  ← EC2 Start/Stop/Reboot
 ├── FormSubmit(FormContext)                  ← S3バケット作成、シークレット作成/更新
 ├── DangerAction(DangerAction)              ← 削除/終了操作
 ├── StartSsoLogin { profile_name, region }  ← SSO loginプロセス起動
 ├── SsmConnect { instance_id }              ← SSM Session Manager接続
 └── EcsExec { cluster_arn, task_arn, container_name }  ← ECS Execute Command
```

### 1.4 Mode（UIモード）

```
Mode
 ├── Normal                                 ← 通常操作
 ├── Filter                                 ← フィルタ入力中
 ├── Confirm(ConfirmAction)                 ← 確認ダイアログ
 │    ├── Start(instance_id)
 │    ├── Stop(instance_id)
 │    └── Reboot(instance_id)
 ├── Message                                ← メッセージ表示
 ├── Help                                   ← ヘルプポップアップ
 ├── Form(FormContext)                      ← フォーム入力
 │    ├── kind: CreateS3Bucket / CreateSecret / UpdateSecretValue
 │    ├── fields: Vec<FormField>
 │    └── focused_field: usize
 ├── DangerConfirm(DangerConfirmContext)    ← 危険操作確認
 │    ├── action: TerminateEc2 / DeleteS3Bucket / DeleteS3Object / DeleteSecret
 │    └── input: Input（確認テキスト入力）
 └── ContainerSelect { names, selected, purpose }  ← コンテナ選択
      └── purpose: ShowLogs / EcsExec
```

---

## 2. App 状態管理

### 2.1 App 構造体

```rust
pub struct App {
    // グローバル UI
    pub should_quit: bool,
    pub message: Option<Message>,       // モーダルメッセージ (level, title, body)
    pub show_help: bool,

    // AWS コンテキスト
    pub profile: Option<String>,        // 選択中のSSOプロファイル名
    pub region: Option<String>,         // リージョン

    // タブ管理
    pub tabs: Vec<Tab>,
    pub active_tab_index: usize,
    next_tab_id: u32,                   // TabId生成カウンタ

    // 画面状態
    pub show_dashboard: bool,
    pub dashboard: DashboardState,

    // オーバーレイ
    pub profile_selector: Option<ProfileSelectorState>,
    pub service_picker: Option<ServicePickerState>,

    // 権限
    pub delete_permissions: DeletePermissions,

    // 一時状態
    pub pending_log_configs: Option<(TabId, Vec<ContainerLogConfig>)>,

    // 非同期チャネル
    pub event_tx: mpsc::Sender<AppEvent>,
    pub event_rx: mpsc::Receiver<AppEvent>,
}
```

### 2.2 dispatch() の処理フロー

```
dispatch(action: Action) → SideEffect

  1. グローバルオーバーレイ処理
     ├── message.is_some() + DismissMessage → message = None
     ├── show_help + (Esc/Back/?/ShowHelp) → show_help = false
     └── その他 → return Noop

  2. プロファイル選択画面
     └── profile_selector → プロファイル選択/フィルタ操作

  3. サービスピッカー
     └── service_picker → サービス選択/フィルタ操作

  4. ダッシュボード
     └── show_dashboard → サービス選択/フィルタ/ナビゲーション

  5. タブ固有モーダル
     ├── Confirm → y: 実行, n: キャンセル
     ├── Form → Submit/Cancel/NextField/入力
     ├── DangerConfirm → Submit(テキスト一致)/Cancel/入力
     └── ContainerSelect → 選択/確定/キャンセル

  6. タブ操作
     ├── NextTab → active_tab_index + 1
     ├── PrevTab → active_tab_index - 1
     ├── CloseTab → tabs.remove() → ダッシュボード復帰
     └── NewTab → service_picker を開く

  7. 通常操作
     ├── Quit → should_quit = true
     ├── ShowHelp → show_help = true
     ├── MoveUp/Down/ToTop/ToBottom/HalfPage → tab.move_*()
     ├── Enter → tab.handle_enter() → List→Detail遷移
     ├── Back → tab.handle_back() → Detail→List復帰
     ├── Refresh → tab.clear_data() + loading = true
     ├── CopyId → tab.copy_id()
     ├── StartFilter → mode = Filter
     ├── ConfirmFilter → mode = Normal + apply_filter()
     ├── CancelFilter → mode = Normal + reset_filter()
     ├── StartStop → mode = Confirm(Start/Stop)
     ├── Reboot → mode = Confirm(Reboot)
     ├── Create → mode = Form(CreateS3Bucket/CreateSecret)
     ├── Delete → mode = DangerConfirm(Delete*)
     ├── Edit → mode = Form(UpdateSecretValue)
     ├── SwitchDetailTab/PrevDetailTab → tab.switch_detail_tab()
     ├── FollowLink → ナビゲーションスタック + VPC遷移
     ├── ShowLogs → ContainerSelect → LogView設定
     ├── SsmConnect → return SideEffect::SsmConnect
     └── EcsExec → ContainerSelect → return SideEffect::EcsExec
```

### 2.3 handle_event() の処理

```
handle_event(event: AppEvent)

  TabEvent(tab_id, event):
    ├── InstancesLoaded(Ok(v))    → instances.set_items(v), apply_filter()
    ├── RepositoriesLoaded(Ok(v)) → repositories.set_items(v), apply_filter()
    ├── ImagesLoaded(Ok(v))       → images = v
    ├── ClustersLoaded(Ok(v))     → clusters.set_items(v), apply_filter()
    ├── EcsServicesLoaded(Ok(v))  → services = v, nav_level = ClusterDetail
    ├── EcsTasksLoaded(Ok(v))     → tasks = v
    ├── BucketsLoaded(Ok(v))      → buckets.set_items(v), apply_filter()
    ├── ObjectsLoaded(Ok(v))      → objects = v
    ├── VpcsLoaded(Ok(v))         → vpcs.set_items(v), apply_filter()
    ├── SubnetsLoaded(Ok(v))      → subnets = v
    ├── SecretsLoaded(Ok(v))      → secrets.set_items(v), apply_filter()
    ├── SecretDetailLoaded(Ok(v)) → detail = Some(v)
    ├── SecretValueLoaded(Ok(v))  → detail.value = v
    ├── NavigateVpcLoaded(Ok((vpcs, subnets)))
    │                              → vpcs/subnets設定, service=Vpc
    ├── EcsLogConfigsLoaded(Ok(configs))
    │                              → pending_log_configs設定 or LogView直接遷移
    ├── EcsLogEventsLoaded(Ok((events, token)))
    │                              → log_state.events追加, auto_scroll処理
    ├── ActionCompleted(Ok(msg))  → show_message(Success)
    └── *Loaded(Err(e)) / ActionCompleted(Err(e))
                                  → show_message(Error)

  CrudCompleted(tab_id, result):
    ├── Ok(msg) → show_message(Success)
    └── Err(e)  → show_message(Error)

  SsoLoginOutput(line):
    → profile_selector.login_output.push(line)

  SsoLoginCompleted(result):
    ├── Ok((profile, region)) → App初期化, ダッシュボード表示
    └── Err(e) → show_message(Error), logging_in = false
```

---

## 3. タブ詳細設計

### 3.1 Tab 構造体

```rust
pub struct Tab {
    pub id: TabId,                              // 一意識別子
    pub service: ServiceKind,                   // サービス種別
    pub tab_view: TabView,                      // List / Detail
    pub mode: Mode,                             // UIモード
    pub loading: bool,                          // ローディング表示
    pub selected_index: usize,                  // リストビューの選択位置
    pub filter_input: Input,                    // フィルタ入力
    pub detail_tab: DetailTab,                  // Overview / Tags
    pub detail_tag_index: usize,                // 詳細ビュー内の選択位置
    pub data: ServiceData,                      // サービス固有データ
    pub navigation_stack: Vec<NavigationEntry>, // パンくずナビゲーション
    pub navigate_target_id: Option<String>,     // クロスサービス遷移先ID
}
```

### 3.2 FilterableList<T>

```rust
pub struct FilterableList<T: Clone> {
    items: Vec<T>,          // 全アイテム（フィルタ前）
    pub filtered: Vec<T>,   // フィルタ済みアイテム（表示用）
}
```

**メソッド:**
- `set_items(items)` → 全アイテム設定 + filtered 同期
- `apply_filter(pred)` → items から pred で絞り込み
- `reset_filter()` → filtered = items.clone()
- `all()` → 全アイテムへの参照
- `len()` / `is_empty()` → filtered のサイズ

### 3.3 ECS ナビゲーション

ECS は4段階のネストを持つ唯一のサービスである。

```
EcsNavLevel
 ├── ClusterDetail           ← クラスター詳細（サービス一覧）
 ├── ServiceDetail           ← サービス詳細（タスク一覧）
 │    └── service_index
 ├── TaskDetail              ← タスク詳細
 │    ├── service_index
 │    └── task_index
 └── LogView                 ← CloudWatch Logs表示
      ├── service_index
      ├── task_index
      └── log_state: Box<LogViewState>
```

**遷移フロー:**
```
ECS List → Enter → ClusterDetail → Enter → ServiceDetail
                                             → Enter → TaskDetail
                                                         → l → LogView
                                                         → a → EcsExec
```

**Back操作:**
```
LogView → Esc → TaskDetail → Esc → ServiceDetail → Esc → ClusterDetail → Esc → List
```

### 3.4 LogViewState（ログ表示状態）

```rust
pub struct LogViewState {
    pub container_name: String,
    pub log_group: String,
    pub log_stream: String,
    pub events: Vec<LogEvent>,              // ログイベント
    pub next_forward_token: Option<String>, // ページネーショントークン
    pub auto_scroll: bool,                  // 自動スクロール
    pub scroll_offset: usize,              // スクロール位置
    pub search_query: String,              // 検索クエリ
    pub search_matches: Vec<usize>,        // マッチしたイベントインデックス
    pub current_match_index: Option<usize>, // 現在のマッチ位置
}
```

**ポーリング:** ログビュー表示中は2秒間隔で CloudWatch Logs に新しいイベントを問い合わせる。非表示時はポーリングを停止する。

### 3.5 クロスサービスナビゲーション

EC2 詳細ビューから VPC/Subnet へのリンクフォロー:

```
1. EC2 Detail → FollowLink (VPC ID or Subnet ID)
2. NavigationEntry を navigation_stack に push
3. tab.service = Vpc, tab.tab_view = Detail
4. navigate_target_id = "vpc-xxx" or "subnet-xxx"
5. handle_navigation_link():
   ├── VPCクライアント作成（なければ）
   ├── 全VPC取得
   ├── target_id が subnet- で始まる場合
   │   → 全VPCのサブネットを走査して該当VPCを特定
   └── 該当VPCのサブネット取得 → NavigateVpcLoaded
6. Esc → navigation_stack.pop() → EC2 Detail に復帰
```

---

## 4. キーバインド定義

### 4.1 共通ナビゲーション

| キー   | アクション   | コンテキスト    |
| ------ | ------------ | --------------- |
| j / ↓  | MoveDown     | リスト/詳細     |
| k / ↑  | MoveUp       | リスト/詳細     |
| g      | MoveToTop    | リスト/詳細     |
| G      | MoveToBottom | リスト/詳細     |
| Ctrl+d | HalfPageDown | リスト/詳細     |
| Ctrl+u | HalfPageUp   | リスト/詳細     |
| Enter  | Enter        | リスト→詳細遷移 |
| Esc    | Back         | 詳細→リスト復帰 |
| /      | StartFilter  | リストビュー    |
| r      | Refresh      | リストビュー    |
| y      | CopyId       | リスト/詳細     |
| ?      | ShowHelp     | 全コンテキスト  |
| q      | Quit         | Normal モード   |

### 4.2 タブ操作

| キー      | アクション                 |
| --------- | -------------------------- |
| Tab       | NextTab                    |
| Shift+Tab | PrevTab                    |
| Ctrl+t    | NewTab（サービスピッカー） |
| Ctrl+w    | CloseTab                   |

### 4.3 EC2 固有

| キー  | アクション         | コンテキスト                         |
| ----- | ------------------ | ------------------------------------ |
| S     | StartStop          | リスト (running→Stop, stopped→Start) |
| R     | Reboot             | リスト                               |
| D     | Delete (Terminate) | リスト (要 --allow-delete)           |
| s     | SsmConnect         | リスト/詳細                          |
| ]     | SwitchDetailTab    | 詳細                                 |
| [     | PrevDetailTab      | 詳細                                 |
| Enter | FollowLink         | 詳細 (VPC/Subnet リンク)             |

### 4.4 ECS 固有

| キー | アクション          | コンテキスト |
| ---- | ------------------- | ------------ |
| l    | ShowLogs            | タスク詳細   |
| a    | EcsExec             | タスク詳細   |
| f    | LogToggleAutoScroll | ログビュー   |
| n    | LogSearchNext       | ログビュー   |
| N    | LogSearchPrev       | ログビュー   |

### 4.5 S3 固有

| キー  | アクション | コンテキスト                                     |
| ----- | ---------- | ------------------------------------------------ |
| c     | Create     | リスト（バケット作成）                           |
| D     | Delete     | リスト（バケット削除）/ 詳細（オブジェクト削除） |
| Enter | Enter      | 詳細（プレフィックスナビゲーション）             |

### 4.6 Secrets Manager 固有

| キー | アクション        | コンテキスト                           |
| ---- | ----------------- | -------------------------------------- |
| c    | Create            | リスト（シークレット作成）             |
| D    | Delete            | リスト（シークレット削除）             |
| e    | Edit              | 詳細（値の更新）                       |
| ]    | SwitchDetailTab   | 詳細 (Overview→Rotation→Versions→Tags) |
| v    | RevealSecretValue | 詳細（値の表示/非表示）                |

### 4.7 フィルタモード

| キー     | アクション                 |
| -------- | -------------------------- |
| Enter    | ConfirmFilter（確定）      |
| Esc      | CancelFilter（キャンセル） |
| 文字入力 | FilterHandleInput          |

### 4.8 確認ダイアログ

| キー    | アクション |
| ------- | ---------- |
| y       | ConfirmYes |
| n / Esc | ConfirmNo  |

### 4.9 フォームダイアログ

| キー     | アクション      |
| -------- | --------------- |
| Enter    | FormSubmit      |
| Esc      | FormCancel      |
| Tab      | FormNextField   |
| 文字入力 | FormHandleInput |

---

## 5. TUI コンポーネント設計

### 5.1 レイアウトパターン

全ビューに共通するレイアウト構造:

```
┌─ タイトル ── プロファイル | リージョン ──────────────┐
│                                                      │
│  （コンテンツ領域）                                  │
│                                                      │
└──────────────────────────────────────────────────────┘
 j/k:移動 Enter:選択 /:フィルタ r:更新 ?:ヘルプ q:終了
```

```rust
// 外枠 + ステータスバー分割
let outer_chunks = Layout::vertical([
    Constraint::Min(1),      // コンテンツ（枠線含む）
    Constraint::Length(1),   // ステータスバー
]).split(area);

// 外枠レンダリング
let outer_block = Block::default()
    .title(left_title)
    .title(right_title.alignment(Alignment::Right))
    .borders(Borders::ALL);
let inner = outer_block.inner(outer_chunks[0]);
frame.render_widget(outer_block, outer_chunks[0]);

// コンテンツを inner に配置
// ...

// フッター
render_footer(frame, outer_chunks[1], &keybinds, filter_input, mode);
```

### 5.2 コンポーネント一覧

#### SelectableTable

行選択可能なテーブルウィジェット。

**入力:** headers (Row), rows (Vec\<Row\>), widths (Vec\<Constraint\>), selected_index
**スタイル:** ヘッダー = `theme::header()`, 選択行 = `theme::selected()`

#### TabBar

タブバーウィジェット。2つ以上のタブが存在する場合のみ表示。

```
 [EC2] │ [S3] │ [ECS]
  ^^^           アクティブ: theme::active() (Cyan+Bold)
                非アクティブ: theme::inactive() (DarkGray)
```

#### StatusBar

フッターのキーバインドヒント。Filter モード時はフィルタ入力を表示。

```
Normal: j/k:move Enter:detail /:filter r:refresh ?:help q:quit
Filter: /search_text█
```

#### ConfirmDialog

y/n 確認ダイアログ。`centered_rect(50%, 7行)` で中央配置。

```
┌─ Confirm ────────────────────────┐
│                                  │
│  Stop instance i-0abc1234?       │
│                                  │
│       [Yes (y)]  [No (n)]        │
└──────────────────────────────────┘
```

#### MessageDialog

Info/Success/Error メッセージ。タイトルの色がレベルで変化。

```
┌─ Success ────────────────────────┐  (緑)
│                                  │
│  Bucket 'my-bucket' created      │
│                                  │
│           [OK (Enter)]           │
└──────────────────────────────────┘
```

#### FormDialog

複数フィールドのフォーム入力。フォーカスフィールドにカーソルを表示。

```
┌─ Create S3 Bucket ───────────────┐
│                                  │
│  * Bucket Name                   │
│  ┌──────────────────────────┐    │
│  │my-bucket█                │    │
│  └──────────────────────────┘    │
│                                  │
│  [Submit (Enter)] [Cancel (Esc)] │
│         [Next Field (Tab)]       │
└──────────────────────────────────┘
```

#### DangerConfirmDialog

削除などの危険操作の確認。リソース名を正確に入力しないと Submit できない。

```
┌─ Delete Bucket ──────────────────┐  (赤枠)
│                                    │
│  Type 'my-bucket' to delete:       │
│  ┌──────────────────────────┐      │
│  │my-buck█                  │      │
│  └──────────────────────────┘      │
│                                    │
│  [Submit (Enter)] [Cancel (Esc)]   │
│  ※ Submit は入力が一致するまで無効 │
└────────────────────────────────────┘
```

#### Loading

アニメーションスピナー。6 tick ごとにフレーム更新。

```
⠋ Loading instances...
```

フレーム: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏

#### HelpPopup

コンテキスト認識ヘルプ。サービスとビューに応じて表示するアクションが変わる。

#### ServicePicker

Ctrl+t で開くサービス選択ポップアップ。ファジーフィルタ対応。

```
┌─ Select Service ─────────────────┐
│ /ec█                             │
│ ▶ EC2                            │
│   ECR                            │
│   ECS                            │
└──────────────────────────────────┘
```

### 5.3 テーマ（カラー定義）

| 関数                     | スタイル                  | 用途                               |
| ------------------------ | ------------------------- | ---------------------------------- |
| `active()`               | Cyan + Bold               | アクティブタブ、フォーカスフレーム |
| `inactive()`             | DarkGray                  | 非アクティブタブ                   |
| `selected()`             | White on DarkGray + Bold  | 選択行                             |
| `header()`               | Yellow + Bold             | テーブルヘッダー                   |
| `status_bar()`           | White on DarkGray         | ステータスバー                     |
| `state_running()`        | Green                     | EC2 running 状態                   |
| `state_stopped()`        | Red                       | EC2 stopped 状態                   |
| `state_pending()`        | Yellow                    | EC2 pending/transitioning 状態     |
| `state_terminated()`     | DarkGray                  | EC2 terminated 状態                |
| `error()`                | Red + Bold                | エラーメッセージ                   |
| `success()`              | Green                     | 成功メッセージ                     |
| `info()`                 | Cyan                      | 情報メッセージ                     |
| `search_match()`         | Yellow bg + Black fg      | 検索マッチ                         |
| `search_match_current()` | Cyan bg + Black fg + Bold | 現在の検索マッチ                   |

---

## 6. ビュー詳細設計

### 6.1 ダッシュボード (dashboard.rs)

```
┌─ awsui Dashboard ── profile | region ──────────────┐
│                                                    │
│  Recently Used                                     │
│  ────────────                                      │
│  ▶ EC2                                             │
│    S3                                              │
│                                                    │
│  All Services                                      │
│  ────────────                                      │
│    ECR                                             │
│    ECS                                             │

│    VPC                                             │
│    Secrets Manager                                 │
│                                                    │
└────────────────────────────────────────────────────┘
 j/k:select /:filter Enter:open q:quit
```

**DashboardState:**
- `selected_index` — Recently Used + All Services を通しインデックスで管理
- `recent_services` — 最近使用したサービス（ファイルに永続化）
- `filtered_services` — フィルタ後の全サービス

### 6.2 プロファイル選択 (profile_select.rs)

```
┌─ Select Profile ───────────────────────────────────┐
│  Name               Region            URL          │
│  ──────────────────────────────────────────────    │
│  ▶ dev-account      ap-northeast-1    https://...  │
│    staging          us-east-1         https://...  │
│    production       ap-northeast-1    https://...  │
└────────────────────────────────────────────────────┘
 j/k:move Enter:select /:filter g/G:top/bottom
```

**SSO Login ダイアログ（オーバーレイ）:**
```
┌─ SSO Login ──────────────────────────────────┐
│  Attempting to open auth page...             │
│  https://device.sso.ap-northeast-1.aws...    │
│                                              │
│  ⠙ Waiting for authentication...             │
│                                              │
│  [Esc] Cancel                                │
└──────────────────────────────────────────────┘
```

### 6.3 EC2 リスト (ec2_list.rs)

**テーブルカラム:**

| カラム      | 幅      | 内容                    |
| ----------- | ------- | ----------------------- |
| Instance ID | 20      | i-0abc1234def           |
| Name        | 15      | web-server-01           |
| State       | 14      | アイコン + 状態テキスト |
| Type        | 12      | t3.micro                |
| AZ          | Min(10) | ap-northeast-1a         |

**State アイコン:**
- running: `▲` (Green)
- stopped: `■` (Red)
- pending/stopping/...: `●` (Yellow)
- terminated: `✕` (DarkGray)

### 6.4 EC2 詳細 (ec2_detail.rs)

```
┌─ web-server (i-0abc1234) ── profile | region ──────┐
│  [Overview]  [Tags]                                │
│ ─────────────────────────────────────────────────  │
│ ┌─ Instance ──────────┐  ┌─ Network ─────────────┐ │
│ │ ID:    i-0abc1234   │  │ VPC:    vpc-123 →     │ │
│ │ Name:  web-server   │  │ Subnet: subnet-456 →  │ │
│ │ Type:  t3.micro     │  │ PrvIP:  10.0.1.100    │ │
│ │ State: ▲ running    │  │ PubIP:  54.210.12.34  │ │
│ │ AZ:    ap-north-1a  │  │ SG:     sg-789abc     │ │
│ │ AMI:   ami-0abc123  │  └───────────────────────┘ │
│ └─────────────────────┘                            │
│ ┌─ Storage ──────────────────────────────────────┐ │
│ │ Volume ID   Type  Size  Device                 │ │
│ │ vol-abc123  gp3   20GB  /dev/xvda              │ │
│ └────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────┘
 ]/[:tab j/k:select Enter:follow-link s:SSM Esc:back
```

**リンク可能フィールド:** VPC ID, Subnet ID（Enter でクロスサービスナビゲーション）

**詳細タブ:**
- Overview: Instance情報 + Network情報 + Storage テーブル
- Tags: Key/Value テーブル

### 6.5 ECS ログビュー (ecs_log.rs)

```
┌─ Logs: my-container ─────────────────────────────┐
│ 2025-01-10 10:30:15  Starting application...     │
│ 2025-01-10 10:30:16  Loading configuration...    │
│ 2025-01-10 10:30:17  Server ready on port 8080   │
│ 2025-01-10 10:31:02  GET / → 200 (12ms)          │
│ 2025-01-10 10:31:05  GET /api/health → 200 (3ms) │
└──────────────────────────────────────────────────┘
 [LIVE] 45/126 j/k:scroll f:pause /:search n/N:next/prev
```

**機能:**
- LIVE/PAUSED モード（`f` で切替）
- 検索（`/` で入力、`n`/`N` でマッチ間移動）
- 自動スクロール（LIVE 時は新着ログに追従）
- マルチラインメッセージのインデント表示
- 検索ハイライト（Yellow=マッチ、Cyan+Bold=現在のマッチ）

### 6.6 S3 詳細 (s3_detail.rs)

**プレフィックスナビゲーション:**
```
バケットルート → images/ → images/2025/ → images/2025/01/
                 (Enter)    (Enter)        (Enter)
                 (Esc で一つ上に戻る)
```

### 6.7 Secrets Manager 詳細 (secrets_detail.rs)

**4つの詳細タブ:**
1. Overview — ARN、説明、作成日、最終更新日、値（マスク/表示切替）
2. Rotation — ローテーション設定
3. Versions — バージョンステージ一覧
4. Tags — タグ Key/Value テーブル

---

## 7. AWS クライアント詳細設計

### 7.1 クライアントトレイト一覧

#### Ec2Client

```rust
async fn describe_instances(&self) -> Result<Vec<Instance>, AppError>;
async fn start_instances(&self, ids: &[String]) -> Result<(), AppError>;
async fn stop_instances(&self, ids: &[String]) -> Result<(), AppError>;
async fn reboot_instances(&self, ids: &[String]) -> Result<(), AppError>;
async fn terminate_instances(&self, ids: &[String]) -> Result<(), AppError>;
```

#### EcrClient

```rust
async fn describe_repositories(&self) -> Result<Vec<Repository>, AppError>;
async fn list_images(&self, repository_name: &str) -> Result<Vec<Image>, AppError>;
```

#### EcsClient

```rust
async fn list_clusters(&self) -> Result<Vec<Cluster>, AppError>;
async fn list_services(&self, cluster_arn: &str) -> Result<Vec<Service>, AppError>;
async fn list_tasks(&self, cluster_arn: &str, service_name: &str) -> Result<Vec<Task>, AppError>;
async fn describe_task_definition_log_configs(&self, task_def_arn: &str) -> Result<Vec<ContainerLogConfig>, AppError>;
```

#### S3Client

```rust
async fn list_buckets(&self) -> Result<Vec<Bucket>, AppError>;
async fn list_objects(&self, bucket: &str, prefix: Option<String>) -> Result<Vec<S3Object>, AppError>;
async fn create_bucket(&self, name: &str) -> Result<(), AppError>;
async fn delete_bucket(&self, name: &str) -> Result<(), AppError>;
async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), AppError>;
```

#### VpcClient

```rust
async fn describe_vpcs(&self) -> Result<Vec<Vpc>, AppError>;
async fn describe_subnets(&self, vpc_id: &str) -> Result<Vec<Subnet>, AppError>;
```

#### SecretsClient

```rust
async fn list_secrets(&self) -> Result<Vec<Secret>, AppError>;
async fn describe_secret(&self, secret_id: &str) -> Result<SecretDetail, AppError>;
async fn get_secret_value(&self, secret_id: &str) -> Result<String, AppError>;
async fn create_secret(&self, name: &str, value: &str, description: Option<String>) -> Result<(), AppError>;
async fn update_secret_value(&self, secret_id: &str, value: &str) -> Result<(), AppError>;
async fn delete_secret(&self, name: &str) -> Result<(), AppError>;
```

#### LogsClient

```rust
async fn get_log_events(&self, log_group: &str, log_stream: &str, next_token: Option<String>) -> Result<(Vec<LogEvent>, Option<String>), AppError>;
```

### 7.2 ドメインモデル

#### EC2

```rust
pub struct Instance {
    pub instance_id: String,
    pub name: String,
    pub instance_type: String,
    pub state: InstanceState,
    pub availability_zone: String,
    pub ami_id: String,
    pub vpc_id: String,
    pub subnet_id: String,
    pub private_ip: String,
    pub public_ip: Option<String>,
    pub security_groups: Vec<String>,
    pub tags: Vec<(String, String)>,
    pub volumes: Vec<Volume>,
}

pub enum InstanceState {
    Running, Stopped, Pending, Stopping,
    ShuttingDown, Terminated, Unknown(String),
}

pub struct Volume {
    pub volume_id: String,
    pub volume_type: String,
    pub size_gb: i32,
    pub device_name: String,
}
```

#### ECR

```rust
pub struct Repository {
    pub repository_name: String,
    pub repository_uri: String,
    pub created_at: String,
    pub image_count: i32,
}

pub struct Image {
    pub image_digest: String,
    pub image_tags: Vec<String>,
    pub pushed_at: String,
    pub size_bytes: i64,
}
```

#### ECS

```rust
pub struct Cluster {
    pub cluster_name: String,
    pub cluster_arn: String,
    pub status: String,
    pub running_tasks: i32,
    pub pending_tasks: i32,
    pub active_services: i32,
    pub registered_instances: i32,
}

pub struct Service {
    pub service_name: String,
    pub service_arn: String,
    pub status: String,
    pub desired_count: i32,
    pub running_count: i32,
    pub launch_type: String,
    pub task_definition: String,
}

pub struct Task {
    pub task_arn: String,
    pub task_definition_arn: String,
    pub last_status: String,
    pub desired_status: String,
    pub started_at: Option<String>,
    pub container_names: Vec<String>,
    pub launch_type: String,
}

pub struct ContainerLogConfig {
    pub container_name: String,
    pub log_group: String,
    pub log_stream_prefix: String,
}
```

#### S3

```rust
pub struct Bucket {
    pub name: String,
    pub creation_date: String,
}

pub struct S3Object {
    pub key: String,
    pub size: i64,
    pub last_modified: String,
    pub is_prefix: bool, // ディレクトリの場合 true
}
```

#### VPC

```rust
pub struct Vpc {
    pub vpc_id: String,
    pub name: String,
    pub cidr_block: String,
    pub state: String,
    pub is_default: bool,
}

pub struct Subnet {
    pub subnet_id: String,
    pub name: String,
    pub cidr_block: String,
    pub availability_zone: String,
    pub available_ips: i32,
    pub state: String,
}
```

#### Secrets Manager

```rust
pub struct Secret {
    pub name: String,
    pub arn: String,
    pub description: String,
    pub last_changed_date: String,
}

pub struct SecretDetail {
    pub arn: String,
    pub name: String,
    pub description: String,
    pub created_date: String,
    pub last_changed_date: String,
    pub last_accessed_date: String,
    pub value: Option<String>,
    pub rotation_enabled: bool,
    pub rotation_lambda_arn: Option<String>,
    pub rotation_rules: Option<String>,
    pub version_stages: Vec<(String, Vec<String>)>,
    pub tags: Vec<(String, String)>,
}
```

---

## 8. インフラストラクチャ詳細

### 8.1 AWS Config パース (config.rs)

**2フェーズパース:**

```
フェーズ1: [sso-session xxx] セクションを先にパース
  → HashMap<session_name, SsoSessionInfo { sso_start_url, sso_region }>

フェーズ2: [profile xxx] セクションをパース
  ├── 直接形式: sso_start_url を直接指定
  │   [profile dev]
  │   sso_start_url = https://...
  │   sso_region = ap-northeast-1
  │
  └── 間接形式: sso_session で参照
      [profile dev]
      sso_session = my-sso     ← フェーズ1のマップから解決
      region = ap-northeast-1
```

**SsoProfile 構造体:**

```rust
pub struct SsoProfile {
    pub name: String,
    pub region: Option<String>,
    pub sso_start_url: String,
    pub sso_session: Option<String>,
}
```

### 8.2 SSO トークン検証 (sso.rs)

**キャッシュファイルパス:** `~/.aws/sso/cache/{SHA1}.json`
- SHA1 のソース: `sso_session` 名、または `sso_start_url`

**トークン検証フロー:**
```
1. キャッシュファイル存在チェック
   └── NotFound → SsoTokenStatus::NotFound

2. ファイル内容から expiresAt を抽出
   └── カスタム RFC3339 パーサー（JSON パーサー不使用）

3. 有効期限チェック
   ├── expires_at > now → SsoTokenStatus::Valid
   └── expires_at ≤ now → SsoTokenStatus::Expired
```

**RFC3339 パーサー:**
- `"2026-02-07T04:16:52Z"` と `"2026-02-07T04:16:52UTC"` の両方に対応
- Howard Hinnant のアルゴリズムによる days_from_civil() で Unix エポック変換

### 8.3 ファジー検索 (fuzzy.rs)

- **ライブラリ:** nucleo
- **設定:** CaseMatching::Ignore, Normalization::Smart
- **NAME_BONUS (1000):** 名前フィールドにボーナスを付与して優先表示
- **入力:** アイテムリスト、クエリ文字列、名前フィールドインデックス、フィールド抽出関数
- **出力:** スコア順でソートされたフィルタ済みアイテム

### 8.4 最近使用サービス (recent.rs)

- **永続化先:** `~/.config/awsui/recent.json`
- **最大数:** 10（MAX_RECENT）
- **更新:** 使用サービスを先頭に移動、重複排除

### 8.5 エラーハンドリング (error.rs)

```rust
pub enum AppError {
    AwsApi(String),   // AWS SDK エラー
    Config(String),   // 設定ファイルエラー
    Io(io::Error),    // IOエラー
}
```

`format_error_chain()` により、エラーの source chain を `: ` で連結して表示する。

---

## 9. テスト戦略

### テスト種別と数

| カテゴリ   | テスト数 | 内容                             |
| ---------- | -------- | -------------------------------- |
| config     | 9        | SSO プロファイルパース           |
| app        | 50+      | dispatch/handle_event の状態遷移 |
| aws        | 22       | クライアントトレイト動作         |
| input      | 30       | キーマッピング                   |
| components | 26       | コンポーネントレンダリング       |
| views      | 23+      | ビュースナップショット           |
| cli        | 12       | CLI引数パース                    |
| sso        | 8        | SSOトークン検証                  |
| fuzzy      | 8        | ファジー検索                     |
| recent     | 4        | 最近使用サービス                 |

### テスト手法

- **mockall:** AWS クライアントトレイトの自動モック生成。`expect_*()` で呼出期待値を設定
- **insta:** UI スナップショットテスト。`cargo insta accept` でスナップショット承認
- **rstest:** パラメータ化テスト。`#[case]` で複数ケースを1テストに集約
- **pretty_assertions:** 失敗時のアサーション出力を見やすく表示

### テストパターン

```rust
// Appのdispatchテスト
#[test]
fn dispatch_move_down_increments_index_when_items_exist() {
    let mut app = test_app_with_ec2_tab();
    app.dispatch(Action::MoveDown);
    assert_eq!(app.active_tab().unwrap().selected_index, 1);
}

// AWS クライアントモック
#[tokio::test]
async fn describe_instances_returns_instances_when_api_succeeds() {
    let mut mock = MockEc2Client::new();
    mock.expect_describe_instances()
        .returning(|| Ok(vec![test_instance()]));
    let result = mock.describe_instances().await;
    assert_eq!(result.unwrap().len(), 1);
}
```

---

## 10. CI/CD

### ci.yaml

```
fmt → clippy → build → nextest (Linux/macOS/Windows)
                                  └→ coverage (Linux, octocov)
```

### audit.yaml

依存関係のセキュリティ監査。

### release.yaml

Git タグ push 時にクロスプラットフォームバイナリを GoReleaser でビルド・配布。
