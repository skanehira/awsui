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

### Event-Driven Architecture
```
KeyEvent (crossterm) → handle_key() → Action → app.dispatch()
                                                  ├→ Sync state update
                                                  └→ tokio::spawn async AWS call
                                                       ↓ mpsc channel
                                                     AppEvent → app.handle_event() → state update
                                                       ↓
                                                     terminal.draw() @ 16ms interval (60 FPS)
```

- `Action` (action.rs): UI actions produced by key input
- `AppEvent` (event.rs): Async events from background AWS API calls, sent via `mpsc` channel
- `App` (app.rs): Central state holding all UI state, AWS context, and per-service data

### Module Structure
```
src/
├── main.rs          # Entry point, terminal init, tokio::select! main loop
├── app.rs           # App state, Mode/View enums, dispatch/handle_event
├── action.rs        # Action enum (UI actions from key input)
├── event.rs         # AppEvent enum (async AWS results)
├── error.rs         # AppError (thiserror)
├── config.rs        # AWS config (~/.aws/config) SSO profile parsing
├── aws/             # AWS SDK integration (trait per service)
│   ├── client.rs    # Ec2Client trait + AwsEc2Client
│   ├── ecr_client.rs / ecs_client.rs / s3_client.rs / vpc_client.rs / secrets_client.rs
│   ├── model.rs     # EC2 domain models
│   └── ecr_model.rs / ecs_model.rs / s3_model.rs / vpc_model.rs / secrets_model.rs
└── tui/             # UI layer
    ├── input.rs     # Key event → Action mapping (context-aware by Mode/View)
    ├── theme.rs     # Color palette and styles
    ├── components/  # Reusable widgets (table, dialog, help, loading, status_bar, list_selector)
    └── views/       # Screen views (profile_select, service_select, ec2/ecr/ecs/s3/vpc/secrets list+detail)
```

### Key Patterns

**Trait-based AWS clients**: Each service has a trait (e.g., `Ec2Client`) with `#[cfg_attr(test, automock)]` for mockall. Concrete implementations wrap the AWS SDK.

**Navigation flow**: ProfileSelect → ServiceSelect → List view → Detail view. `Esc` navigates back, `q` quits.

**State management**: `App` holds unfiltered data (`instances`), filtered data (`filtered_instances`), and selection indices. Modes (`Normal`, `Filter`, `Confirm`, `Message`, `Help`) control input context.

**TUI rendering**: All views use `Block::default().borders(Borders::ALL)` for outer frame. Pattern: outer_chunks → outer_block.inner() → content layout inside.

### Testing
- **mockall**: Auto-generated mocks for AWS client traits
- **insta**: Snapshot tests for UI rendering
- **rstest**: Parameterized tests with `#[case]`
- **pretty_assertions**: Readable assertion diffs

### CI/CD
- **ci.yaml**: fmt + clippy + build + nextest (Linux/macOS/Windows), coverage on Linux via octocov
- **audit.yaml**: Dependency security audit
- **release.yaml**: Cross-platform release on tag push (GoReleaser)
