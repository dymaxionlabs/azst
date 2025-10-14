use anyhow::{anyhow, Result};
use std::path::Path;

/// Parse an Azure storage URI (az://storage_account/container/path) into components
/// Returns (storage_account, container, blob_path)
///
/// Formats supported:
/// - az://account/container/path/to/blob -> (Some(account), container, Some(path/to/blob))
/// - az://account/container/ -> (Some(account), container, None)
/// - az://account/container -> (Some(account), container, None)
/// - az://container/path (legacy) -> (None, container, Some(path))
/// - az://container/ (legacy) -> (None, container, None)
pub fn parse_azure_uri(uri: &str) -> Result<(Option<String>, String, Option<String>)> {
    if !uri.starts_with("az://") {
        return Err(anyhow!("Invalid Azure URI. Must start with 'az://'"));
    }

    let path_part = &uri[5..]; // Remove "az://" prefix
    let parts: Vec<&str> = path_part.splitn(3, '/').collect();

    if parts.is_empty() || parts[0].is_empty() {
        return Err(anyhow!(
            "Invalid Azure URI. Storage account or container name is required"
        ));
    }

    // Check if this is the new format (account/container/path) or legacy (container/path)
    // Heuristic: if we have 2+ parts and the first part looks like a storage account name
    // (lowercase, no underscores, etc.), assume new format
    // Special case: if we have only 1 part and it looks like a storage account, treat it as account
    if parts.len() == 1 && is_storage_account_name(parts[0]) {
        // Single component that looks like storage account: az://account or az://account/
        // This means: list all containers in this account
        let storage_account = Some(parts[0].to_string());
        let container = String::new(); // Empty container means list all containers
        Ok((storage_account, container, None))
    } else if parts.len() >= 2 && is_storage_account_name(parts[0]) {
        // New format: az://account/container/path
        let storage_account = Some(parts[0].to_string());
        let container = parts[1].to_string();
        let blob_path = if parts.len() > 2 && !parts[2].is_empty() {
            Some(parts[2].to_string())
        } else {
            None
        };
        Ok((storage_account, container, blob_path))
    } else {
        // Legacy format: az://container/path or ambiguous case
        let container = parts[0].to_string();
        let blob_path = if parts.len() > 1 {
            // Join all remaining parts to form the full path
            let remaining_parts = &parts[1..];
            let joined = remaining_parts.join("/");
            if !joined.is_empty() {
                Some(joined)
            } else {
                None
            }
        } else {
            None
        };
        Ok((None, container, blob_path))
    }
}

/// Check if a string looks like a storage account name
/// Storage account names: 3-24 chars, lowercase letters and numbers only
fn is_storage_account_name(s: &str) -> bool {
    let len = s.len();
    (3..=24).contains(&len)
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Check if a path is an Azure storage URI
pub fn is_azure_uri(path: &str) -> bool {
    path.starts_with("az://")
}

/// Format file size in human readable format
pub fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Get the filename from a path (works with both local and Azure paths)
pub fn get_filename(path: &str) -> String {
    if is_azure_uri(path) {
        if let Ok((_, _, Some(blob_path))) = parse_azure_uri(path) {
            Path::new(&blob_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&blob_path)
                .to_string()
        } else {
            "".to_string()
        }
    } else {
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path)
            .to_string()
    }
}

/// Check if a local path is a directory
pub fn is_directory(path: &str) -> bool {
    Path::new(path).is_dir()
}

/// Check if a local path exists
pub fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// Get the parent directory of a path
pub fn get_parent_dir(path: &str) -> Option<String> {
    Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
}

/// Normalize a path by removing trailing slashes (except for root)
#[allow(dead_code)]
pub fn normalize_path(path: &str) -> String {
    if path == "/" {
        return path.to_string();
    }
    path.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_azure_uri_new_format() {
        // New format with storage account
        let (account, container, path) =
            parse_azure_uri("az://myaccount/mycontainer/path/to/file.txt").unwrap();
        assert_eq!(account, Some("myaccount".to_string()));
        assert_eq!(container, "mycontainer");
        assert_eq!(path, Some("path/to/file.txt".to_string()));

        let (account, container, path) = parse_azure_uri("az://myaccount/mycontainer/").unwrap();
        assert_eq!(account, Some("myaccount".to_string()));
        assert_eq!(container, "mycontainer");
        assert_eq!(path, None);

        let (account, container, path) = parse_azure_uri("az://myaccount/mycontainer").unwrap();
        assert_eq!(account, Some("myaccount".to_string()));
        assert_eq!(container, "mycontainer");
        assert_eq!(path, None);

        // Storage account with numbers
        let (account, container, _) =
            parse_azure_uri("az://samaindevoptimus/dev/uploads/file.txt").unwrap();
        assert_eq!(account, Some("samaindevoptimus".to_string()));
        assert_eq!(container, "dev");

        // Just storage account - should list all containers
        let (account, container, path) = parse_azure_uri("az://samaindevoptimus").unwrap();
        assert_eq!(account, Some("samaindevoptimus".to_string()));
        assert_eq!(container, ""); // Empty means list all containers
        assert_eq!(path, None);

        let (account, container, path) = parse_azure_uri("az://samaindevoptimus/").unwrap();
        assert_eq!(account, Some("samaindevoptimus".to_string()));
        assert_eq!(container, ""); // Empty means list all containers
        assert_eq!(path, None);
    }

    #[test]
    fn test_parse_azure_uri_legacy_format() {
        // Legacy format without storage account
        let (account, container, path) =
            parse_azure_uri("az://MyContainer/path/to/file.txt").unwrap();
        assert_eq!(account, None);
        assert_eq!(container, "MyContainer");
        assert_eq!(path, Some("path/to/file.txt".to_string()));

        let (account, container, path) = parse_azure_uri("az://MyContainer/").unwrap();
        assert_eq!(account, None);
        assert_eq!(container, "MyContainer");
        assert_eq!(path, None);
    }

    #[test]
    fn test_parse_azure_uri_invalid() {
        assert!(parse_azure_uri("invalid://uri").is_err());
        assert!(parse_azure_uri("az://").is_err());
    }

    #[test]
    fn test_is_storage_account_name() {
        assert!(is_storage_account_name("myaccount"));
        assert!(is_storage_account_name("account123"));
        assert!(is_storage_account_name("samaindevoptimus"));
        assert!(!is_storage_account_name("MyAccount")); // uppercase
        assert!(!is_storage_account_name("my_account")); // underscore
        assert!(!is_storage_account_name("ab")); // too short
        assert!(!is_storage_account_name("a".repeat(25).as_str())); // too long
    }

    #[test]
    fn test_is_azure_uri() {
        assert!(is_azure_uri("az://container/path"));
        assert!(!is_azure_uri("/local/path"));
        assert!(!is_azure_uri("gs://bucket/path"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
        assert_eq!(format_size(1099511627776), "1.0 TB");
        assert_eq!(format_size(0), "0 B");
    }

    #[test]
    fn test_get_filename() {
        // Local paths
        assert_eq!(get_filename("/path/to/file.txt"), "file.txt");
        assert_eq!(get_filename("/path/to/dir/"), "dir");
        assert_eq!(get_filename("file.txt"), "file.txt");
        assert_eq!(get_filename("/"), "/"); // Root returns itself

        // Azure URIs
        assert_eq!(
            get_filename("az://account/container/path/to/file.txt"),
            "file.txt"
        );
        assert_eq!(get_filename("az://account/container/file.txt"), "file.txt");
        assert_eq!(get_filename("az://account/container/"), "");
        assert_eq!(get_filename("az://account/container"), "");
    }

    #[test]
    fn test_get_parent_dir() {
        assert_eq!(
            get_parent_dir("/path/to/file.txt"),
            Some("/path/to".to_string())
        );
        assert_eq!(get_parent_dir("/path/to/"), Some("/path".to_string()));
        assert_eq!(get_parent_dir("file.txt"), Some("".to_string()));
        assert_eq!(get_parent_dir("/"), None);
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/path/to/file/"), "/path/to/file");
        assert_eq!(normalize_path("/path/to/file"), "/path/to/file");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path("//"), ""); // Multiple slashes trimmed
    }

    #[test]
    fn test_parse_azure_uri_edge_cases() {
        // Empty path components
        let (account, container, path) = parse_azure_uri("az://account/container//").unwrap();
        assert_eq!(account, Some("account".to_string()));
        assert_eq!(container, "container");
        assert_eq!(path, Some("/".to_string())); // Double slash results in "/"

        // Deep nested paths
        let (account, container, path) =
            parse_azure_uri("az://myaccount/mycontainer/a/b/c/d/e/file.txt").unwrap();
        assert_eq!(account, Some("myaccount".to_string()));
        assert_eq!(container, "mycontainer");
        assert_eq!(path, Some("a/b/c/d/e/file.txt".to_string()));

        // Special characters in blob name
        let (account, container, path) =
            parse_azure_uri("az://myaccount/mycontainer/file-name_2024.txt").unwrap();
        assert_eq!(account, Some("myaccount".to_string()));
        assert_eq!(container, "mycontainer");
        assert_eq!(path, Some("file-name_2024.txt".to_string()));
    }

    #[test]
    fn test_is_storage_account_name_edge_cases() {
        // Boundary cases
        assert!(is_storage_account_name("abc")); // exactly 3 chars
        assert!(is_storage_account_name("abcdefghij1234567890abcd")); // exactly 24 chars
        assert!(!is_storage_account_name("ab")); // too short
        assert!(!is_storage_account_name("abcdefghij1234567890abcde")); // too long

        // Mixed valid characters
        assert!(is_storage_account_name("abc123"));
        assert!(is_storage_account_name("123abc"));
        assert!(is_storage_account_name("a1b2c3"));

        // Invalid characters
        assert!(!is_storage_account_name("abc-123")); // hyphen
        assert!(!is_storage_account_name("abc.123")); // dot
        assert!(!is_storage_account_name("abc 123")); // space
        assert!(!is_storage_account_name("ABC")); // uppercase
    }
}
