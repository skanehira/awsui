use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

/// mock-data featureでビルドされたバイナリのパス（テスト全体で1回だけビルド）
///
/// 環境変数 `AWSUI_MOCK_BINARY` が設定されていればそのパスを使用。
/// 設定されていなければ `cargo build --features mock-data` を実行してビルドする。
/// テスト実行と feature flag 切り替えによる再ビルドの競合を避けるため、
/// 別の target ディレクトリ（target/e2e）にビルドする。
static MOCK_BINARY: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(path) = std::env::var("AWSUI_MOCK_BINARY") {
        let p = PathBuf::from(path);
        assert!(
            p.exists(),
            "AWSUI_MOCK_BINARY path does not exist: {}",
            p.display()
        );
        return p;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir.join("target/e2e");

    let status = std::process::Command::new("cargo")
        .args(["build", "--features", "mock-data", "--target-dir"])
        .arg(&target_dir)
        .status()
        .expect("Failed to build mock binary");
    assert!(status.success(), "Mock binary build failed");
    target_dir.join("debug/awsui")
});

/// PTYベースのテストハーネス
pub struct TestHarness {
    parser: vt100::Parser,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    timeout: Duration,
}

#[allow(dead_code)]
impl TestHarness {
    /// 画面状態を更新（PTYから読み取り可能なデータをすべて処理）
    pub fn update(&mut self) {
        let mut buf = [0u8; 4096];
        // ノンブロッキング読み取りのために小さなループ
        loop {
            match self.reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => self.parser.process(&buf[..n]),
                Err(_) => break,
            }
        }
    }

    /// 画面内容をテキストとして取得
    pub fn screen_contents(&mut self) -> String {
        self.update();
        self.parser.screen().contents()
    }

    /// 指定テキストが画面に出現するまで待つ
    pub fn wait_for_text(&mut self, text: &str) -> Result<(), String> {
        let start = Instant::now();
        loop {
            let contents = self.screen_contents();
            if contents.contains(text) {
                return Ok(());
            }
            if start.elapsed() > self.timeout {
                return Err(format!(
                    "Timeout waiting for '{}'. Screen contents:\n{}",
                    text, contents
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// テキストを送信
    pub fn send_text(&mut self, text: &str) -> Result<(), std::io::Error> {
        self.writer.write_all(text.as_bytes())?;
        self.writer.flush()
    }

    /// Enterキーを送信
    pub fn send_enter(&mut self) -> Result<(), std::io::Error> {
        self.send_text("\r")
    }

    /// Escキーを送信
    pub fn send_esc(&mut self) -> Result<(), std::io::Error> {
        self.send_text("\x1b")
    }

    /// 上矢印キーを送信
    pub fn send_up(&mut self) -> Result<(), std::io::Error> {
        self.send_text("\x1b[A")
    }

    /// 下矢印キーを送信
    pub fn send_down(&mut self) -> Result<(), std::io::Error> {
        self.send_text("\x1b[B")
    }

    /// Tabキーを送信
    pub fn send_tab(&mut self) -> Result<(), std::io::Error> {
        self.send_text("\t")
    }

    /// Ctrl+キーを送信
    pub fn send_ctrl(&mut self, ch: char) -> Result<(), std::io::Error> {
        let ctrl_byte = (ch as u8) & 0x1f;
        self.writer.write_all(&[ctrl_byte])?;
        self.writer.flush()
    }

    /// 文字キーを送信
    pub fn send_char(&mut self, ch: char) -> Result<(), std::io::Error> {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        self.send_text(s)
    }

    /// '/' キーを送信（フィルター開始）
    pub fn send_slash(&mut self) -> Result<(), std::io::Error> {
        self.send_char('/')
    }

    /// プロセス終了を待つ
    pub fn wait_exit(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        self.child.wait()
    }
}

/// テスト用ハーネスを起動するヘルパー
pub fn spawn_app(cols: u16, rows: u16) -> TestHarness {
    let pty_system = NativePtySystem::default();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to open PTY");

    // HOMEを一時ディレクトリに設定し、recent.jsonが存在しない状態で起動する。
    // これにより "Recently Used" セクションが空になり、ダッシュボードの選択が
    // "All Services" の先頭（EC2）から始まる。
    let temp_home = std::env::temp_dir().join(format!(
        "awsui-e2e-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_home).expect("Failed to create temp home dir");

    let mut cmd = CommandBuilder::new(&*MOCK_BINARY);
    cmd.args(["--profile", "mock-profile"]);
    cmd.env("TERM", "xterm-256color");
    cmd.env("HOME", temp_home.to_str().unwrap());

    let child = pair.slave.spawn_command(cmd).expect("Failed to spawn app");
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .expect("Failed to clone reader");
    let writer = pair.master.take_writer().expect("Failed to take writer");

    // reader をノンブロッキングにする
    // portable-pty の reader は blocking なので、別スレッドで読み取る
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();

    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // チャネルベースのreaderをラップ
    let channel_reader = ChannelReader { rx };

    TestHarness {
        parser: vt100::Parser::new(rows, cols, 0),
        reader: Box::new(channel_reader),
        writer: Box::new(writer),
        child,
        timeout: Duration::from_secs(10),
    }
}

/// mpscチャネルからデータを読むReader
struct ChannelReader {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.rx.try_recv() {
            Ok(data) => {
                let len = data.len().min(buf.len());
                buf[..len].copy_from_slice(&data[..len]);
                Ok(len)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "no data",
            )),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Ok(0),
        }
    }
}
