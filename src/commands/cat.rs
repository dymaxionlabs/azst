use anyhow::{anyhow, Result};
use colored::*;
use std::io::Write;

use crate::azure::AzureClient;
use crate::utils::{is_azure_uri, parse_azure_uri};

pub struct CatOptions<'a> {
    pub urls: &'a [String],
    pub header: bool,
    pub range: Option<&'a str>,
}

pub async fn execute(urls: &[String], header: bool, range: Option<&str>) -> Result<()> {
    let options = CatOptions {
        urls,
        header,
        range,
    };
    execute_with_options(options).await
}

async fn execute_with_options(options: CatOptions<'_>) -> Result<()> {
    if options.urls.is_empty() {
        return Err(anyhow!("No URLs provided"));
    }

    // Process each URL
    for (idx, url) in options.urls.iter().enumerate() {
        if !is_azure_uri(url) {
            return Err(anyhow!(
                "Invalid URL '{}'. Must be an Azure URL (az://container/path)",
                url
            ));
        }

        // Print header if requested (and if multiple files, or if header flag is set)
        let should_print_header = options.header;

        if should_print_header && idx > 0 {
            // Add a blank line between files
            eprintln!();
        }

        if should_print_header {
            eprintln!("==> {} <==", url.cyan());
        }

        // Download to stdout
        if options.range.is_some() {
            download_with_range(url, options.range).await?;
        } else {
            download_to_stdout(url).await?;
        }
    }

    Ok(())
}

async fn download_to_stdout(display_url: &str) -> Result<()> {
    // Parse account, container and blob from the az:// URL
    let (account_opt, container, blob_path_opt) = parse_azure_uri(display_url)?;

    let blob =
        blob_path_opt.ok_or_else(|| anyhow!("No blob path specified in URL '{}'", display_url))?;

    // Create Azure client
    let mut azure_client = AzureClient::new();
    if let Some(account_name) = account_opt {
        azure_client = azure_client.with_storage_account(&account_name);
    }
    azure_client.check_prerequisites().await?;

    // Download blob content
    let content = azure_client
        .download_blob(&container, &blob, None)
        .await
        .map_err(|e| {
            // Provide user-friendly error messages
            let err_str = e.to_string();
            if err_str.contains("BlobNotFound") || err_str.contains("does not exist") {
                anyhow!(
                    "Blob '{}' not found in container '{}'. Please verify the blob path.",
                    blob,
                    container
                )
            } else if err_str.contains("ContainerNotFound") {
                anyhow!(
                    "Container '{}' does not exist. Please verify the container name.",
                    container
                )
            } else {
                e
            }
        })?;

    // Write to stdout
    std::io::stdout()
        .write_all(&content)
        .map_err(|e| anyhow!("Failed to write to stdout: {}", e))?;

    Ok(())
}

async fn download_with_range(display_url: &str, range: Option<&str>) -> Result<()> {
    let range_str = range.ok_or_else(|| anyhow!("Range is required"))?;

    // Parse account, container and blob from the az:// URL
    let (account_opt, container, blob_path_opt) = parse_azure_uri(display_url)?;

    let blob =
        blob_path_opt.ok_or_else(|| anyhow!("No blob path specified in URL '{}'", display_url))?;

    // Convert range format to Azure's format
    let azure_range = parse_range(range_str)?;

    // Create Azure client
    let mut azure_client = AzureClient::new();
    if let Some(account_name) = account_opt {
        azure_client = azure_client.with_storage_account(&account_name);
    }
    azure_client.check_prerequisites().await?;

    // Download blob content with range
    let content = if let Some((start, end)) = azure_range {
        azure_client
            .download_blob(&container, &blob, Some((start, end.unwrap_or(u64::MAX))))
            .await?
    } else {
        azure_client.download_blob(&container, &blob, None).await?
    };

    // Write to stdout
    std::io::stdout()
        .write_all(&content)
        .map_err(|e| anyhow!("Failed to write to stdout: {}", e))?;

    Ok(())
}

/// Parse range string in gsutil format and convert to (start, end) bytes
/// Formats: "start-end", "start-", "-numbytes"
fn parse_range(range: &str) -> Result<Option<(u64, Option<u64>)>> {
    if range.starts_with('-') {
        // Last N bytes format: "-5" means last 5 bytes
        // Azure CLI doesn't support negative offsets directly
        // We would need to get the blob size first
        return Err(anyhow!(
            "Negative byte range (-N) not yet supported. Use start-end or start- format."
        ));
    }

    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid range format. Use 'start-end', 'start-', or '-numbytes'"
        ));
    }

    let start: u64 = parts[0]
        .parse()
        .map_err(|_| anyhow!("Invalid start byte offset"))?;

    let end = if parts[1].is_empty() {
        None
    } else {
        Some(
            parts[1]
                .parse()
                .map_err(|_| anyhow!("Invalid end byte offset"))?,
        )
    };

    Ok(Some((start, end)))
}
