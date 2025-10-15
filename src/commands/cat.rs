use anyhow::{anyhow, Result};
use colored::*;
use std::io::Write;

use crate::azure::AzCopyClient;
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

        // We don't need the HTTPS URL anymore since we're using the parsed URI components

        // Check AzCopy prerequisites (only once)
        if idx == 0 {
            let mut azcopy = AzCopyClient::new();
            azcopy.check_prerequisites().await?;
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
    use tokio::fs;
    use tokio::process::Command;

    // Parse account, container and blob from the az:// URL
    let (account_opt, container, blob_path_opt) = parse_azure_uri(display_url)?;

    let blob =
        blob_path_opt.ok_or_else(|| anyhow!("No blob path specified in URL '{}'", display_url))?;

    // Create a temporary file for downloading
    let temp_file = format!("/tmp/azst_cat_{}", std::process::id());

    // Use az storage blob download with a temporary file
    let mut cmd = Command::new("az");
    cmd.args([
        "storage",
        "blob",
        "download",
        "--container-name",
        &container,
        "--name",
        &blob,
        "--file",
        &temp_file,
        "--no-progress", // Disable progress bar
        "--auth-mode",
        "login", // Use Azure AD authentication
        "--output",
        "none", // Disable JSON output formatting
    ]);

    // Get storage account from the parsed URI
    if let Some(ref account) = account_opt {
        cmd.args(["--account-name", account]);
    }

    // Command is ready to execute

    let output = cmd.output().await.context_download_failed()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Process the error message to provide helpful feedback

        // Parse common errors and provide user-friendly messages
        if stderr.contains("The specified blob does not exist") || stderr.contains("BlobNotFound") {
            return Err(anyhow!(
                "Blob '{}' not found in container '{}'. Please verify the blob path.",
                blob,
                container
            ));
        } else if stderr.contains("The specified container does not exist")
            || stderr.contains("ContainerNotFound")
            || (stderr.contains("container") && stderr.contains("not found"))
        {
            return Err(anyhow!(
                "Container '{}' does not exist. Please verify the container name.",
                container
            ));
        } else if stderr.contains("Storage account") && stderr.contains("not found") {
            if let Some(ref account) = account_opt {
                return Err(anyhow!(
                    "Storage account '{}' not found. Please verify the account name and ensure you have access to it.",
                    account
                ));
            }
        } else if stderr.contains("not logged in") || stderr.contains("Please run 'az login'") {
            return Err(anyhow!(
                "Not logged in to Azure. Please run 'az login' first."
            ));
        }

        return Err(anyhow!("Failed to download blob: {}", stderr));
    }

    // Read the temporary file and write to stdout
    match fs::read(&temp_file).await {
        Ok(content) => {
            std::io::stdout()
                .write_all(&content)
                .context_download_failed()?;
        }
        Err(e) => {
            return Err(anyhow!(
                "Failed to read temporary file '{}': {}",
                temp_file,
                e
            ));
        }
    }

    // Clean up temporary file
    if let Err(e) = fs::remove_file(&temp_file).await {
        eprintln!(
            "Warning: Failed to clean up temporary file '{}': {}",
            temp_file, e
        );
    }

    Ok(())
}

async fn download_with_range(display_url: &str, range: Option<&str>) -> Result<()> {
    use tokio::fs;
    use tokio::process::Command;

    let range_str = range.ok_or_else(|| anyhow!("Range is required"))?;

    // Parse account, container and blob from the az:// URL
    let (account_opt, container, blob_path_opt) = parse_azure_uri(display_url)?;

    let blob =
        blob_path_opt.ok_or_else(|| anyhow!("No blob path specified in URL '{}'", display_url))?;

    // Convert range format to Azure's format
    let azure_range = parse_range(range_str)?;

    // Create a temporary file for downloading
    let temp_file = format!("/tmp/azst_cat_range_{}", std::process::id());

    let mut cmd = Command::new("az");
    cmd.args([
        "storage",
        "blob",
        "download",
        "--container-name",
        &container,
        "--name",
        &blob,
        "--file",
        &temp_file,      // Use temporary file instead of stdout
        "--no-progress", // Disable progress bar
        "--auth-mode",
        "login", // Use Azure AD authentication
        "--output",
        "none", // Disable JSON output formatting
    ]);

    // Add range if specified
    if let Some((start, end)) = azure_range {
        cmd.args(["--start-range", &start.to_string()]);
        if let Some(end_byte) = end {
            cmd.args(["--end-range", &end_byte.to_string()]);
        }
    }

    // Get storage account from the parsed URI
    if let Some(ref account) = account_opt {
        cmd.args(["--account-name", account]);
    }

    let output = cmd.output().await.context_download_failed()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse common errors and provide user-friendly messages
        if stderr.contains("The specified blob does not exist") || stderr.contains("BlobNotFound") {
            return Err(anyhow!(
                "Blob '{}' not found in container '{}'. Please verify the blob path.",
                blob,
                container
            ));
        } else if stderr.contains("The specified container does not exist")
            || stderr.contains("ContainerNotFound")
            || (stderr.contains("container") && stderr.contains("not found"))
        {
            return Err(anyhow!(
                "Container '{}' does not exist. Please verify the container name.",
                container
            ));
        } else if stderr.contains("Storage account") && stderr.contains("not found") {
            if let Some(ref account) = account_opt {
                return Err(anyhow!(
                    "Storage account '{}' not found. Please verify the account name and ensure you have access to it.",
                    account
                ));
            }
        } else if stderr.contains("not logged in") || stderr.contains("Please run 'az login'") {
            return Err(anyhow!(
                "Not logged in to Azure. Please run 'az login' first."
            ));
        }

        return Err(anyhow!("Failed to download blob with range: {}", stderr));
    }

    // Read the temporary file and write to stdout
    match fs::read(&temp_file).await {
        Ok(content) => {
            std::io::stdout()
                .write_all(&content)
                .context_download_failed()?;
        }
        Err(e) => {
            return Err(anyhow!(
                "Failed to read temporary file '{}': {}",
                temp_file,
                e
            ));
        }
    }

    // Clean up temporary file
    if let Err(e) = fs::remove_file(&temp_file).await {
        eprintln!(
            "Warning: Failed to clean up temporary file '{}': {}",
            temp_file, e
        );
    }

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

trait ContextExt<T> {
    fn context_download_failed(self) -> Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ContextExt<T> for Result<T, E> {
    fn context_download_failed(self) -> Result<T> {
        self.map_err(|e| anyhow!("Download failed: {}", e))
    }
}
