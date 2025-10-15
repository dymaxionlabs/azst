use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::azure::{AzureClient, BlobItem};
use crate::output::create_writer;
use crate::utils::{format_size, is_azure_uri, parse_azure_uri};

/// Execute the disk usage command
pub async fn execute(
    path: Option<&str>,
    summarize: bool,
    human_readable: bool,
    total: bool,
    account: Option<&str>,
) -> Result<()> {
    match path {
        Some(p) if is_azure_uri(p) => {
            let mut azure_client = AzureClient::new();
            if let Some(account_name) = account {
                azure_client = azure_client.with_storage_account(account_name);
            }
            azure_client.check_prerequisites().await?;
            calculate_azure_usage(p, summarize, human_readable, total, &azure_client).await
        }
        Some(p) => calculate_local_usage(p, summarize, human_readable, total).await,
        None => Err(anyhow!("Path is required for du command")),
    }
}

async fn calculate_azure_usage(
    path: &str,
    summarize: bool,
    human_readable: bool,
    total: bool,
    azure_client: &AzureClient,
) -> Result<()> {
    let (account, container, prefix) = parse_azure_uri(path)?;

    // Create azure client with account if specified in URI
    let client = if let Some(account_name) = account.clone() {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    // Get the actual account name being used
    let actual_account = client
        .get_storage_account()
        .ok_or_else(|| anyhow!("Storage account not configured"))?;

    // Special case: If we have an account but no container, calculate usage for all containers
    if account.is_some() && container.is_empty() {
        return calculate_all_containers_usage(summarize, human_readable, total, &client).await;
    }

    // List all blobs recursively (no delimiter)
    let blobs = client
        .list_blobs(&container, prefix.as_deref(), None)
        .await?;

    if summarize {
        // Calculate total size only
        let total_size = calculate_total_size(&blobs);
        let size_str = if human_readable {
            format_size(total_size)
        } else {
            total_size.to_string()
        };

        let display_path = format!(
            "az://{}/{}{}",
            actual_account,
            container,
            prefix.as_deref().unwrap_or("")
        );
        println!("{}\t{}", size_str, display_path);
    } else {
        // Calculate size for each directory level
        let dir_sizes = calculate_directory_sizes(&blobs, prefix.as_deref());

        // Sort by path for consistent output
        let mut sorted_dirs: Vec<_> = dir_sizes.iter().collect();
        sorted_dirs.sort_by(|a, b| a.0.cmp(b.0));

        let writer = create_writer();

        for (dir_path, size) in sorted_dirs {
            let size_str = if human_readable {
                format_size(*size)
            } else {
                size.to_string()
            };

            let display_path = format!("az://{}/{}/{}", actual_account, container, dir_path);
            writer.write_disk_usage(&size_str, &display_path);
        }

        // Print total if requested
        if total {
            let total_size = calculate_total_size(&blobs);
            let size_str = if human_readable {
                format_size(total_size)
            } else {
                total_size.to_string()
            };
            let display_path = format!(
                "az://{}/{}{}",
                actual_account,
                container,
                prefix.as_deref().unwrap_or("")
            );
            writer.write_disk_usage_total(&size_str, &display_path);
        }
    }

    Ok(())
}

async fn calculate_all_containers_usage(
    summarize: bool,
    human_readable: bool,
    total: bool,
    client: &AzureClient,
) -> Result<()> {
    let containers = client.list_containers().await?;

    if containers.is_empty() {
        println!("No containers found");
        return Ok(());
    }

    let actual_account = client
        .get_storage_account()
        .ok_or_else(|| anyhow!("Storage account not configured"))?;

    let writer = create_writer();
    let mut grand_total: u64 = 0;

    for container in containers {
        let blobs = client.list_blobs(&container.name, None, None).await?;
        let container_size = calculate_total_size(&blobs);
        grand_total += container_size;

        if !summarize {
            let size_str = if human_readable {
                format_size(container_size)
            } else {
                container_size.to_string()
            };
            let display_path = format!("az://{}/{}/", actual_account, container.name);
            writer.write_disk_usage(&size_str, &display_path);
        }
    }

    if summarize || total {
        let size_str = if human_readable {
            format_size(grand_total)
        } else {
            grand_total.to_string()
        };
        let display_path = format!("az://{}/", actual_account);
        if summarize {
            writer.write_disk_usage(&size_str, &display_path);
        } else {
            writer.write_disk_usage_total(&size_str, &display_path);
        }
    }

    Ok(())
}

fn calculate_total_size(blobs: &[BlobItem]) -> u64 {
    blobs
        .iter()
        .map(|item| match item {
            BlobItem::Blob(blob) => blob.properties.content_length,
            BlobItem::Prefix(_) => 0, // Prefixes don't have size
        })
        .sum()
}

fn calculate_directory_sizes(
    blobs: &[BlobItem],
    base_prefix: Option<&str>,
) -> HashMap<String, u64> {
    let mut dir_sizes: HashMap<String, u64> = HashMap::new();

    for item in blobs {
        if let BlobItem::Blob(blob) = item {
            let size = blob.properties.content_length;

            // Get the relative path (strip base prefix if present)
            let relative_path = if let Some(prefix) = base_prefix {
                blob.name.strip_prefix(prefix).unwrap_or(&blob.name)
            } else {
                &blob.name
            };

            // Split the path into segments and accumulate sizes for each directory level
            let segments: Vec<&str> = relative_path.split('/').collect();

            // Add size to each directory level
            // For path "a/b/c/file.txt", add to "a/", "a/b/", "a/b/c/"
            for i in 1..segments.len() {
                let dir_path = segments[..i].join("/") + "/";
                *dir_sizes.entry(dir_path).or_insert(0) += size;
            }
        }
    }

    dir_sizes
}

async fn calculate_local_usage(
    path: &str,
    summarize: bool,
    human_readable: bool,
    total: bool,
) -> Result<()> {
    use std::path::Path;
    use tokio::fs;

    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(anyhow!("Path '{}' does not exist", path));
    }

    if path_obj.is_file() {
        // Single file - just show its size
        let metadata = fs::metadata(path).await?;
        let size = metadata.len();
        let size_str = if human_readable {
            format_size(size)
        } else {
            size.to_string()
        };
        println!("{}\t{}", size_str, path);
        return Ok(());
    }

    if !path_obj.is_dir() {
        return Err(anyhow!("Path '{}' is neither file nor directory", path));
    }

    // Calculate directory sizes
    let dir_sizes = calculate_local_directory_sizes(path, summarize).await?;

    let writer = create_writer();

    if summarize {
        // Just show the total for the main directory
        if let Some(total_size) = dir_sizes.get(path) {
            let size_str = if human_readable {
                format_size(*total_size)
            } else {
                total_size.to_string()
            };
            writer.write_disk_usage(&size_str, path);
        }
    } else {
        // Show all subdirectories
        let mut sorted_dirs: Vec<_> = dir_sizes.iter().collect();
        sorted_dirs.sort_by(|a, b| a.0.cmp(b.0));

        for (dir_path, size) in sorted_dirs {
            let size_str = if human_readable {
                format_size(*size)
            } else {
                size.to_string()
            };
            writer.write_disk_usage(&size_str, dir_path);
        }

        // Print total if requested
        if total {
            if let Some(total_size) = dir_sizes.get(path) {
                let size_str = if human_readable {
                    format_size(*total_size)
                } else {
                    total_size.to_string()
                };
                writer.write_disk_usage_total(&size_str, path);
            }
        }
    }

    Ok(())
}

async fn calculate_local_directory_sizes(
    root_path: &str,
    summarize_only: bool,
) -> Result<HashMap<String, u64>> {
    use std::path::Path;
    use tokio::fs;

    let mut dir_sizes: HashMap<String, u64> = HashMap::new();

    // Recursive function to traverse directory tree
    fn traverse_dir<'a>(
        dir_path: &'a Path,
        root: &'a Path,
        dir_sizes: &'a mut HashMap<String, u64>,
        summarize_only: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            let mut total_size: u64 = 0;
            let mut entries = fs::read_dir(dir_path).await?;

            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                let metadata = entry.metadata().await?;

                if metadata.is_file() {
                    total_size += metadata.len();
                } else if metadata.is_dir() {
                    // Recursively calculate subdirectory size
                    let subdir_size =
                        traverse_dir(&entry_path, root, dir_sizes, summarize_only).await?;
                    total_size += subdir_size;

                    // Store this subdirectory's size unless we're only summarizing the root
                    if !summarize_only {
                        if let Some(path_str) = entry_path.to_str() {
                            dir_sizes.insert(path_str.to_string(), subdir_size);
                        }
                    }
                }
            }

            Ok(total_size)
        })
    }

    let root = Path::new(root_path);
    let total_size = traverse_dir(root, root, &mut dir_sizes, summarize_only).await?;

    // Always store the root directory's total size
    dir_sizes.insert(root_path.to_string(), total_size);

    Ok(dir_sizes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_du_container_docs() {
        // Test case: azst du az://account/container/
        // Expected: Show total size of all objects in container
    }

    #[test]
    fn test_du_prefix_docs() {
        // Test case: azst du az://account/container/prefix/
        // Expected: Show size of all objects under prefix
    }

    #[test]
    fn test_du_summarize_docs() {
        // Test case: azst du -s az://account/container/
        // Expected: Show only total size for the container
    }

    #[test]
    fn test_du_human_readable_docs() {
        // Test case: azst du -h az://account/container/
        // Expected: Display sizes in KB/MB/GB format
    }

    #[test]
    fn test_du_detailed_docs() {
        // Test case: azst du az://account/container/
        // Expected: Show size for each subdirectory
    }

    #[test]
    fn test_du_local_directory_docs() {
        // Test case: azst du /local/dir/
        // Expected: Show disk usage for local directory
    }
}
