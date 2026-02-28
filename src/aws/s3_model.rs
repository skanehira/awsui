/// S3バケットのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    pub name: String,
    pub creation_date: Option<String>,
}

/// S3バケット詳細のタブ種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum S3DetailTab {
    Objects,
    Settings,
}

/// S3バケットの設定情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketSettings {
    pub region: String,
    pub versioning: String,
    pub encryption: String,
}

/// S3オブジェクトの内容（プレビュー用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectContent {
    pub content_type: String,
    pub body: String,
    pub size: u64,
}

/// 拡張子またはContent-Typeからテキストファイルかどうかを判定する
pub fn is_text_file(key: &str, content_type: &str) -> bool {
    // 拡張子での判定を優先
    if let Some(ext) = key.rsplit('.').next() {
        let ext_lower = ext.to_lowercase();
        let text_extensions = [
            "json",
            "yaml",
            "yml",
            "txt",
            "log",
            "csv",
            "xml",
            "toml",
            "md",
            "ini",
            "cfg",
            "conf",
            "sh",
            "py",
            "rs",
            "js",
            "ts",
            "html",
            "css",
            "sql",
            "env",
            "gitignore",
            "dockerfile",
            "makefile",
            "tf",
            "hcl",
        ];
        if text_extensions.contains(&ext_lower.as_str()) {
            return true;
        }
    }

    // Content-Type でのフォールバック
    content_type.starts_with("text/")
        || content_type == "application/json"
        || content_type == "application/xml"
        || content_type == "application/yaml"
        || content_type == "application/x-yaml"
        || content_type == "application/toml"
}

/// ダウンロード先のファイルパスを構築する。
/// save_dirが存在するディレクトリかチェックし、object_keyからbasenameを抽出して結合する。
pub fn resolve_download_path(
    save_dir: &str,
    object_key: &str,
) -> Result<std::path::PathBuf, String> {
    use std::path::{Component, Path};

    // パストラバーサル検出: キー内に".."コンポーネントがあれば拒否
    for component in Path::new(object_key).components() {
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "Path traversal detected in object key: {}",
                object_key
            ));
        }
    }

    // basenameを抽出（"folder/file.txt" → "file.txt"）
    let basename = Path::new(object_key)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "Cannot extract filename from object key".to_string())?;

    let dir = Path::new(save_dir);
    if !dir.exists() {
        return Err(format!("Directory '{}' does not exist", save_dir));
    }
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", save_dir));
    }

    Ok(dir.join(basename))
}

/// アップロード先のS3キーを構築する。
/// current_prefixとlocal_file_pathのbasenameを結合する。
pub fn resolve_upload_key(current_prefix: &str, local_file_path: &str) -> Result<String, String> {
    use std::path::Path;

    let basename = Path::new(local_file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "Cannot extract filename from local file path".to_string())?;

    Ok(format!("{}{}", current_prefix, basename))
}

/// S3オブジェクトのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Object {
    pub key: String,
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub storage_class: Option<String>,
    pub is_prefix: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("config.json", "application/octet-stream", true)]
    #[case("data.yaml", "application/octet-stream", true)]
    #[case("data.yml", "application/octet-stream", true)]
    #[case("readme.txt", "application/octet-stream", true)]
    #[case("app.log", "application/octet-stream", true)]
    #[case("data.csv", "application/octet-stream", true)]
    #[case("template.xml", "application/octet-stream", true)]
    #[case("config.toml", "application/octet-stream", true)]
    #[case("readme.md", "application/octet-stream", true)]
    #[case("main.rs", "application/octet-stream", true)]
    #[case("script.sh", "application/octet-stream", true)]
    #[case("main.tf", "application/octet-stream", true)]
    #[case("image.png", "image/png", false)]
    #[case("archive.tar.gz", "application/gzip", false)]
    #[case("binary", "application/octet-stream", false)]
    fn is_text_file_returns_expected_when_extension(
        #[case] key: &str,
        #[case] content_type: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(is_text_file(key, content_type), expected);
    }

    #[rstest]
    #[case("noext", "text/plain", true)]
    #[case("noext", "text/html", true)]
    #[case("noext", "application/json", true)]
    #[case("noext", "application/xml", true)]
    #[case("noext", "application/yaml", true)]
    #[case("noext", "application/octet-stream", false)]
    #[case("noext", "image/jpeg", false)]
    fn is_text_file_returns_expected_when_content_type_fallback(
        #[case] key: &str,
        #[case] content_type: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(is_text_file(key, content_type), expected);
    }

    #[test]
    fn resolve_download_path_returns_path_when_valid_dir_and_key() {
        let dir = std::env::temp_dir();
        let result = resolve_download_path(dir.to_str().unwrap(), "folder/readme.txt");
        assert_eq!(result, Ok(dir.join("readme.txt")));
    }

    #[test]
    fn resolve_download_path_returns_path_when_key_has_no_prefix() {
        let dir = std::env::temp_dir();
        let result = resolve_download_path(dir.to_str().unwrap(), "readme.txt");
        assert_eq!(result, Ok(dir.join("readme.txt")));
    }

    #[test]
    fn resolve_download_path_returns_error_when_dir_does_not_exist() {
        let result = resolve_download_path("/nonexistent/path/abc123", "file.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn resolve_download_path_returns_error_when_key_is_empty() {
        let dir = std::env::temp_dir();
        let result = resolve_download_path(dir.to_str().unwrap(), "");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_download_path_returns_error_when_key_contains_path_traversal() {
        let dir = std::env::temp_dir();
        let result = resolve_download_path(dir.to_str().unwrap(), "../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }

    #[test]
    fn resolve_download_path_returns_error_when_basename_is_dot_dot() {
        let dir = std::env::temp_dir();
        let result = resolve_download_path(dir.to_str().unwrap(), "folder/..");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_upload_key_returns_prefixed_key_when_prefix_exists() {
        let result = resolve_upload_key("folder/subfolder/", "/home/user/image.png");
        assert_eq!(result, Ok("folder/subfolder/image.png".to_string()));
    }

    #[test]
    fn resolve_upload_key_returns_basename_when_prefix_is_empty() {
        let result = resolve_upload_key("", "/home/user/data.csv");
        assert_eq!(result, Ok("data.csv".to_string()));
    }

    #[test]
    fn resolve_upload_key_returns_error_when_file_path_is_empty() {
        let result = resolve_upload_key("folder/", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot extract filename"));
    }

    #[test]
    fn resolve_upload_key_returns_key_when_file_has_no_directory() {
        let result = resolve_upload_key("prefix/", "document.pdf");
        assert_eq!(result, Ok("prefix/document.pdf".to_string()));
    }
}
