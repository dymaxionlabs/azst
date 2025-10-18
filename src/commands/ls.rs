use anyhow::{anyhow, Result};

use crate::azure::{AzureClient, BlobItem};
use crate::output::create_writer;
use crate::utils::{
    contains_recursive_wildcard, format_size, is_azure_uri, matches_pattern, parse_azure_uri,
    split_wildcard_path,
};

/// Calculate the depth of a pattern (number of path segments)
/// Treats ** as matching any depth
fn pattern_depth(pattern: &str) -> Option<usize> {
    if pattern.contains("**") {
        None // Infinite depth
    } else {
        // Count the number of path segments
        Some(pattern.split('/').filter(|s| !s.is_empty()).count())
    }
}

pub async fn execute(
    path: Option<&str>,
    long: bool,
    human_readable: bool,
    recursive: bool,
    account: Option<&str>,
) -> Result<()> {
    match path {
        Some(p) if is_azure_uri(p) => {
            let mut azure_client = AzureClient::new();
            if let Some(account_name) = account {
                azure_client = azure_client.with_storage_account(account_name);
            }
            azure_client.check_prerequisites().await?;
            list_azure_objects(p, long, human_readable, recursive, &mut azure_client).await
        }
        Some(p) => list_local_path(p, long, human_readable, recursive).await,
        None => {
            // List all storage accounts - requires Azure
            let mut azure_client = AzureClient::new();
            azure_client.check_prerequisites().await?;
            list_storage_accounts(long, &mut azure_client).await
        }
    }
}

async fn list_storage_accounts(long: bool, azure_client: &mut AzureClient) -> Result<()> {
    let accounts = azure_client.list_storage_accounts().await?;

    if accounts.is_empty() {
        println!("No storage accounts found");
        return Ok(());
    }

    let writer = create_writer();
    writer.write_header("Azure Storage Accounts:");

    for account in accounts {
        writer.write_storage_account(
            &account.name,
            &account.location,
            &account.resource_group,
            long,
        );
    }

    Ok(())
}

async fn list_containers(long: bool, azure_client: &mut AzureClient) -> Result<()> {
    let containers = azure_client.list_containers().await?;

    if containers.is_empty() {
        println!("No containers found");
        return Ok(());
    }

    let writer = create_writer();
    writer.write_header("Azure Storage Containers:");

    // Get the account name from the client
    let account_name = azure_client
        .get_storage_account()
        .ok_or_else(|| anyhow!("Storage account not configured"))?;

    for container in containers {
        writer.write_container(
            account_name,
            &container.name,
            &container.properties.last_modified,
            long,
        );
    }

    Ok(())
}

/// Stream blob results directly without buffering - for non-wildcard listings
async fn list_blobs_streaming(
    client: &mut AzureClient,
    container: &str,
    actual_account: &str,
    prefix: Option<&str>,
    delimiter: Option<&str>,
    long: bool,
    human_readable: bool,
) -> Result<()> {
    let writer = create_writer();
    writer.write_header(&format!(
        "Contents of az://{}/{}:",
        actual_account, container
    ));

    if long {
        writer.write_table_header(&[("Size", 10), ("Type", 15), ("Modified", 20), ("Name", 0)]);
        writer.write_separator(80);
    }

    let mut item_count = 0;

    // Use the callback-based API to process items as they arrive
    client
        .list_blobs_with_callback(container, prefix, delimiter, |items| {
            for item in items {
                item_count += 1;
                match item {
                    BlobItem::Blob(blob) => {
                        let size_str = if human_readable {
                            format_size(blob.properties.content_length)
                        } else {
                            blob.properties.content_length.to_string()
                        };

                        let content_type = blob
                            .properties
                            .content_type
                            .unwrap_or_else(|| "unknown".to_string());

                        let blob_uri =
                            format!("az://{}/{}/{}", actual_account, container, blob.name);

                        writer.write_blob(
                            &blob_uri,
                            &size_str,
                            &content_type,
                            &blob.properties.last_modified,
                            long,
                        );
                    }
                    BlobItem::Prefix(prefix) => {
                        let prefix_uri =
                            format!("az://{}/{}/{}", actual_account, container, prefix);
                        writer.write_prefix(&prefix_uri, long);
                    }
                }
            }
            Ok(())
        })
        .await?;

    if item_count == 0 {
        println!("No objects found in az://{}/{}/", actual_account, container);
    }

    Ok(())
}

async fn list_azure_objects(
    path: &str,
    long: bool,
    human_readable: bool,
    recursive: bool,
    azure_client: &mut AzureClient,
) -> Result<()> {
    let (account, container, prefix) = parse_azure_uri(path)?;

    // Create azure client with account if specified in URI
    let mut client = if let Some(account_name) = account.clone() {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    // Special case: If we have an account but no container (az://account or az://account/),
    // list all containers in that account
    if account.is_some() && container.is_empty() {
        return list_containers(long, &mut client).await;
    }

    //Check if the prefix contains wildcards
    let (list_prefix, pattern, force_recursive) = if let Some(prefix_str) = &prefix {
        if let Some((before_wildcard, mut wildcard_pattern)) = split_wildcard_path(prefix_str) {
            // Has wildcard: list with prefix before wildcard, then filter with pattern
            // If pattern contains **, force recursive listing (no delimiter)
            // Also force recursive if the wildcard pattern contains / (multi-segment pattern)
            let is_recursive =
                contains_recursive_wildcard(&wildcard_pattern) || wildcard_pattern.contains('/');

            // If pattern ends with /, append * to match contents of that directory
            if wildcard_pattern.ends_with('/') {
                wildcard_pattern.push('*');
            }

            (
                if before_wildcard.is_empty() {
                    None
                } else {
                    Some(before_wildcard)
                },
                Some(wildcard_pattern),
                is_recursive,
            )
        } else {
            // No wildcard: use prefix as-is
            (prefix.clone(), None, false)
        }
    } else {
        // No prefix at all
        (None, None, false)
    };

    // Use delimiter for non-recursive listing (hierarchical, like gsutil default behavior)
    // Omit delimiter for recursive listing or when using ** wildcard
    let delimiter = if recursive || force_recursive {
        None
    } else {
        Some("/")
    };

    // Get the actual account name being used
    let actual_account = client
        .get_storage_account()
        .ok_or_else(|| anyhow!("Storage account not configured"))?
        .to_string();

    // If there's no pattern, we can stream results directly without buffering
    if pattern.is_none() {
        return list_blobs_streaming(
            &mut client,
            &container,
            &actual_account,
            list_prefix.as_deref(),
            delimiter,
            long,
            human_readable,
        )
        .await;
    }

    // For patterns, we need to collect and filter all results
    let blobs = client
        .list_blobs(&container, list_prefix.as_deref(), delimiter)
        .await?;

    // Filter blobs if we have a pattern
    let filtered_blobs: Vec<BlobItem> = if let Some(ref pattern_str) = pattern {
        // Calculate the expected depth based on the pattern
        let expected_depth = pattern_depth(pattern_str);

        // If we have a specific depth (not **) and we're NOT in explicit recursive mode,
        // we need to extract directory prefixes at that depth (hierarchical view)
        if let Some(depth) = expected_depth {
            if force_recursive && !recursive {
                // Extract unique prefixes at the target depth (non-recursive mode with multi-segment pattern)
                let mut unique_prefixes = std::collections::HashSet::new();

                for item in &blobs {
                    let name = match item {
                        BlobItem::Blob(blob) => &blob.name,
                        BlobItem::Prefix(prefix) => prefix,
                    };

                    let match_part = if let Some(ref prefix) = list_prefix {
                        name.strip_prefix(prefix).unwrap_or(name)
                    } else {
                        name
                    };

                    // Extract prefix at target depth
                    let segments: Vec<&str> = match_part.split('/').collect();
                    if segments.len() >= depth {
                        let prefix_at_depth = segments[..depth].join("/") + "/";

                        // Check if this prefix matches the pattern
                        if matches_pattern(&prefix_at_depth, pattern_str) {
                            unique_prefixes.insert(prefix_at_depth);
                        }
                    }
                }

                // Convert prefixes to BlobItem::Prefix
                unique_prefixes
                    .into_iter()
                    .map(|prefix| {
                        let full_name = if let Some(ref list_pfx) = list_prefix {
                            format!("{}{}", list_pfx, prefix)
                        } else {
                            prefix
                        };
                        BlobItem::Prefix(full_name)
                    })
                    .collect()
            } else {
                // Regular filtering for non-recursive with delimiter
                blobs
                    .into_iter()
                    .filter(|item| {
                        let name = match item {
                            BlobItem::Blob(blob) => &blob.name,
                            BlobItem::Prefix(prefix) => prefix,
                        };

                        let match_part = if let Some(ref prefix) = list_prefix {
                            name.strip_prefix(prefix).unwrap_or(name)
                        } else {
                            name
                        };

                        matches_pattern(match_part, pattern_str)
                    })
                    .collect()
            }
        } else {
            // ** pattern - show all matches at any depth
            blobs
                .into_iter()
                .filter(|item| {
                    let name = match item {
                        BlobItem::Blob(blob) => &blob.name,
                        BlobItem::Prefix(prefix) => prefix,
                    };

                    let match_part = if let Some(ref prefix) = list_prefix {
                        name.strip_prefix(prefix).unwrap_or(name)
                    } else {
                        name
                    };

                    matches_pattern(match_part, pattern_str)
                })
                .collect()
        }
    } else {
        blobs
    };

    if filtered_blobs.is_empty() {
        if pattern.is_some() {
            println!(
                "No objects matching pattern in az://{}/{}/",
                actual_account, container
            );
        } else {
            println!("No objects found in az://{}/{}/", actual_account, container);
        }
        return Ok(());
    }

    let writer = create_writer();
    writer.write_header(&format!(
        "Contents of az://{}/{}:",
        actual_account, container
    ));

    if long {
        writer.write_table_header(&[("Size", 10), ("Type", 15), ("Modified", 20), ("Name", 0)]);
        writer.write_separator(80);
    }

    for item in filtered_blobs {
        match item {
            BlobItem::Blob(blob) => {
                let size_str = if human_readable {
                    format_size(blob.properties.content_length)
                } else {
                    blob.properties.content_length.to_string()
                };

                let content_type = blob
                    .properties
                    .content_type
                    .unwrap_or_else(|| "unknown".to_string());

                let blob_uri = format!("az://{}/{}/{}", actual_account, container, blob.name);

                writer.write_blob(
                    &blob_uri,
                    &size_str,
                    &content_type,
                    &blob.properties.last_modified,
                    long,
                );
            }
            BlobItem::Prefix(prefix) => {
                // Display directory/prefix with trailing slash
                let prefix_uri = format!("az://{}/{}/{}", actual_account, container, prefix);
                writer.write_prefix(&prefix_uri, long);
            }
        }
    }

    Ok(())
}

async fn list_local_path(
    path: &str,
    long: bool,
    human_readable: bool,
    recursive: bool,
) -> Result<()> {
    use std::path::Path;

    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(anyhow!("Path '{}' does not exist", path));
    }

    if path_obj.is_file() {
        // List single file
        list_single_file(path, long, human_readable).await
    } else if path_obj.is_dir() {
        // List directory contents
        list_directory(path, long, human_readable, recursive).await
    } else {
        Err(anyhow!("Path '{}' is neither file nor directory", path))
    }
}

async fn list_single_file(path: &str, long: bool, human_readable: bool) -> Result<()> {
    use tokio::fs;

    let writer = create_writer();

    if long {
        let metadata = fs::metadata(path).await?;
        let size = metadata.len();
        let size_str = if human_readable {
            format_size(size)
        } else {
            size.to_string()
        };

        writer.write_local_file(path, &size_str, "file", long);
    } else {
        writer.write_local_file(path, "", "file", long);
    }

    Ok(())
}

async fn list_directory(
    dir_path: &str,
    long: bool,
    human_readable: bool,
    recursive: bool,
) -> Result<()> {
    use tokio::fs;

    let writer = create_writer();

    if long {
        writer.write_table_header(&[("Size", 10), ("Type", 10), ("Name", 0)]);
        writer.write_separator(50);
    }

    if recursive {
        list_directory_recursive(dir_path, "", long, human_readable).await
    } else {
        let mut entries = fs::read_dir(dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let name_str = entry_name.to_str().unwrap_or("?");

            if long {
                let metadata = entry.metadata().await?;
                let size = metadata.len();
                let size_str = if human_readable {
                    format_size(size)
                } else {
                    size.to_string()
                };

                let type_str = if metadata.is_dir() { "dir" } else { "file" };
                let display_name = if metadata.is_dir() {
                    format!("{}/", name_str)
                } else {
                    name_str.to_string()
                };

                writer.write_local_file(&display_name, &size_str, type_str, long);
            } else {
                let display_name = if entry_path.is_dir() {
                    format!("{}/", name_str)
                } else {
                    name_str.to_string()
                };
                writer.write_local_file(&display_name, "", "file", long);
            }
        }

        Ok(())
    }
}

fn list_directory_recursive<'a>(
    dir_path: &'a str,
    prefix: &'a str,
    long: bool,
    human_readable: bool,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        use tokio::fs;

        let writer = create_writer();
        let mut entries = fs::read_dir(dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let name_str = entry_name.to_str().unwrap_or("?");
            let full_name = if prefix.is_empty() {
                name_str.to_string()
            } else {
                format!("{}/{}", prefix, name_str)
            };

            if long {
                let metadata = entry.metadata().await?;
                let size = metadata.len();
                let size_str = if human_readable {
                    format_size(size)
                } else {
                    size.to_string()
                };

                let type_str = if metadata.is_dir() { "dir" } else { "file" };
                let display_name = if metadata.is_dir() {
                    format!("{}/", full_name)
                } else {
                    full_name.to_string()
                };

                writer.write_local_file(&display_name, &size_str, type_str, long);
            } else {
                let display_name = if entry_path.is_dir() {
                    format!("{}/", full_name)
                } else {
                    full_name.to_string()
                };
                writer.write_local_file(&display_name, "", "file", long);
            }

            // Recursively list subdirectories
            if entry_path.is_dir() {
                let entry_str = entry_path.to_str().unwrap();
                list_directory_recursive(entry_str, &full_name, long, human_readable).await?;
            }
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_list_containers_docs() {
        // Test case: azst ls
        // Expected: List all containers in default storage account
    }

    #[test]
    fn test_list_container_contents_docs() {
        // Test case: azst ls az://account/container/
        // Expected: List all blobs in container
    }

    #[test]
    fn test_list_with_prefix_docs() {
        // Test case: azst ls az://account/container/prefix/
        // Expected: List blobs starting with prefix
    }

    #[test]
    fn test_list_long_format_docs() {
        // Test case: azst ls -l az://account/container/
        // Expected: Display size, type, modified date, and name
    }

    #[test]
    fn test_list_human_readable_docs() {
        // Test case: azst ls -lh az://account/container/
        // Expected: Display sizes in KB/MB/GB format
    }

    #[test]
    fn test_list_recursive_docs() {
        // Test case: azst ls -r az://account/container/
        // Expected: List all blobs recursively (Azure lists all by default)
    }

    #[test]
    fn test_list_local_file_docs() {
        // Test case: azst ls /local/file.txt
        // Expected: Display file info
    }

    #[test]
    fn test_list_local_directory_docs() {
        // Test case: azst ls /local/dir/
        // Expected: List directory contents
    }

    // Note: Full integration tests would require mocking Azure CLI calls
}
