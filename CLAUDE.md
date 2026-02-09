# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Build & Run
```bash
cargo build
cargo build --release
cargo run
```

### Testing
```bash
# Run all tests (cargo-nextest recommended)
cargo nextest run

# Run all tests (standard)
cargo test

# Run a single test
cargo test test_name

# Accept snapshot updates (insta)
cargo insta accept

# Generate coverage
cargo llvm-cov nextest --lcov --output-path lcov.info
```

### Quality Checks
```bash
cargo fmt -- --check
cargo fmt
cargo clippy
```

## Architecture

### Overview
awsui is a TUI application for managing AWS resources via SSO. Built with ratatui + crossterm + tokio, it supports EC2, ECR, ECS, S3, VPC, and Secrets Manager.

### Key Settings
- **Rust version**: 1.92 (fixed in `rust-toolchain.toml`)
- **Edition**: Rust 2024

### Startup Flow
1. CLI argument parsing (`cli.rs` with clap) + delete permission check
2. Load SSO profiles from `~/.aws/config` (`config.rs`)
3. Profile selection via skim (fullscreen fuzzy picker)
4. SSO token cache validation (`sso.rs`) → `aws sso login` if expired/missing
5. Region resolution + `App` initialization
6. Terminal init (CrosstermBackend) → main loop → terminal restore

### Event-Driven Architecture
```
KeyEvent (crossterm) → handle_key() → Action → app.dispatch()
                                                  ├→ Sync state update (on active tab)
                                                  └→ tokio::spawn async AWS call
                                                       ↓ mpsc channel
                                                     AppEvent → app.handle_event() → state update
                                                       ↓
                                                     terminal.draw() @ 16ms interval (60 FPS)
```

- `Action` (action.rs): UI actions produced by key input (includes tab operations: `NextTab`, `PrevTab`, `NewTab`, `CloseTab`)
- `AppEvent` (event.rs): Async events from background AWS API calls, sent via `mpsc` channel (includes `TabEvent` for per-tab data delivery)
- `App` (app.rs): Central state holding tabs, AWS context, and shared UI state

### Tab-Based UI Architecture
The UI uses a multi-tab model where each tab is an independent service view with its own state:

- `Tab` (tab.rs): Self-contained unit with `TabId`, `ServiceKind`, `TabView` (List/Detail), `Mode`, `ServiceData`, filter state, selection index, and navigation stack
- `ServiceData` (tab.rs): Enum holding service-specific data (instances, repositories, clusters, buckets, vpcs, secrets) per tab
- `App` manages a `Vec<Tab>` with `active_tab_index` — each tab operates independently
- Tab operations: `Tab`/`Shift+Tab` to switch, `Ctrl+t` to open new (via service picker), `Ctrl+w` to close
- Dashboard view shows all open tabs' service summaries

### Module Structure
```
src/
├── main.rs          # Entry point, terminal init, tokio::select! main loop
├── app.rs           # App state, Mode/View enums, dispatch/handle_event
├── tab.rs           # Tab, TabId, TabView, ServiceData — per-tab state
├── action.rs        # Action enum (UI actions from key input)
├── event.rs         # AppEvent enum (async AWS results, TabEvent)
├── error.rs         # AppError (thiserror)
├── config.rs        # AWS config (~/.aws/config) SSO profile parsing
├── sso.rs           # SSO token cache validation (SsoTokenStatus)
├── cli.rs           # CLI argument parsing (clap derive)
├── service.rs       # ServiceKind enum (Ec2, Ecr, Ecs, S3, Vpc, SecretsManager)
├── fuzzy.rs         # Fuzzy filtering with nucleo
├── recent.rs        # Recently used services tracking
├── aws/             # AWS SDK integration (trait per service)
│   ├── client.rs    # Ec2Client trait + AwsEc2Client
│   ├── ecr_client.rs / ecs_client.rs / s3_client.rs / vpc_client.rs / secrets_client.rs
│   ├── model.rs     # EC2 domain models
│   └── ecr_model.rs / ecs_model.rs / s3_model.rs / vpc_model.rs / secrets_model.rs
└── tui/             # UI layer
    ├── input.rs     # Key event → Action mapping (context-aware by Mode/TabView)
    ├── theme.rs     # Color palette and styles
    ├── components/  # Reusable widgets (table, tab_bar, dialog, form_dialog, help, loading, status_bar, list_selector, service_picker)
    └── views/       # Screen views (dashboard, service_select, ec2/ecr/ecs/s3/vpc/secrets list+detail)
```

### Key Patterns

**Trait-based AWS clients**: Each service has a trait (e.g., `Ec2Client`) with `#[cfg_attr(test, automock)]` for mockall. Concrete implementations wrap the AWS SDK.

**Navigation flow**: skim profile select → ServiceSelect → Tab (List → Detail). Cross-resource navigation (e.g., EC2 → VPC) uses a `navigation_stack` per tab for breadcrumb-style back navigation.

**State management**: Each `Tab` holds its own `ServiceData` with unfiltered + filtered lists and selection state. `Mode` (`Normal`, `Filter`, `Confirm`, `Message`, `Help`, `Form`, `DangerConfirm`) is per-tab. `App`-level state covers tabs list, AWS clients, shared config.

**TUI rendering**: All views use `Block::default().borders(Borders::ALL)` for outer frame. Pattern: outer_chunks (frame + status_bar) → outer_block.inner() → tab_bar + content inside. Tab bar is rendered at the top of the inner area.

**Key input priority** (input.rs): Global overlays (message/help/service_picker) → Dashboard → Active tab modals (Confirm/Form/DangerConfirm) → Tab navigation keys → View-specific handlers

**CRUD operations**: S3 bucket creation, Secrets Manager create/edit/delete use `Form` and `DangerConfirm` modes for safe input handling.

### Testing
- **mockall**: Auto-generated mocks for AWS client traits
- **insta**: Snapshot tests for UI rendering
- **rstest**: Parameterized tests with `#[case]`
- **pretty_assertions**: Readable assertion diffs

### CI/CD
- **ci.yaml**: fmt + clippy + build + nextest (Linux/macOS/Windows), coverage on Linux via octocov
- **audit.yaml**: Dependency security audit
- **release.yaml**: Cross-platform release on tag push (GoReleaser)
