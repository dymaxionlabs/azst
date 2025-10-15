use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::process::Command as AsyncCommand;

// ============================================================================
// AzCopy Options - Common options for azcopy operations
// ============================================================================

/// Options for azcopy copy operations
#[derive(Debug, Clone, Default)]
pub struct AzCopyOptions {
    pub recursive: bool,
    pub dry_run: bool,
    pub cap_mbps: Option<f64>,
    pub include_pattern: Option<String>,
    pub exclude_pattern: Option<String>,
}

impl AzCopyOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_cap_mbps(mut self, cap_mbps: Option<f64>) -> Self {
        self.cap_mbps = cap_mbps;
        self
    }

    pub fn with_include_pattern(mut self, pattern: Option<String>) -> Self {
        self.include_pattern = pattern;
        self
    }

    pub fn with_exclude_pattern(mut self, pattern: Option<String>) -> Self {
        self.exclude_pattern = pattern;
        self
    }

    /// Apply common options to a command
    pub fn apply_to_command(&self, cmd: &mut AsyncCommand) {
        if self.recursive {
            cmd.arg("--recursive");
        }

        if self.dry_run {
            cmd.arg("--dry-run");
        }

        if let Some(mbps) = self.cap_mbps {
            cmd.arg(format!("--cap-mbps={}", mbps));
        }

        if let Some(pattern) = &self.include_pattern {
            cmd.arg(format!("--include-pattern={}", pattern));
        }

        if let Some(pattern) = &self.exclude_pattern {
            cmd.arg(format!("--exclude-pattern={}", pattern));
        }
    }
}

// ============================================================================
// Azure Configuration and Data Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct AzureConfig {
    pub storage_account: Option<String>,
    #[allow(dead_code)]
    pub subscription_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BlobInfo {
    pub name: String,
    #[serde(rename = "properties")]
    pub properties: BlobProperties,
}

#[derive(Debug, Deserialize)]
pub struct BlobProperties {
    #[serde(rename = "contentLength")]
    pub content_length: u64,
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    #[serde(rename = "properties")]
    pub properties: ContainerProperties,
}

#[derive(Debug, Deserialize)]
pub struct ContainerProperties {
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    #[serde(rename = "publicAccess")]
    #[allow(dead_code)]
    pub public_access: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StorageAccountInfo {
    pub name: String,
    pub location: String,
    #[serde(rename = "resourceGroup")]
    pub resource_group: String,
    #[serde(rename = "creationTime")]
    #[allow(dead_code)]
    pub creation_time: String,
}

#[derive(Clone)]
pub struct AzureClient {
    config: AzureConfig,
}

impl AzureClient {
    pub fn new() -> Self {
        Self {
            config: AzureConfig {
                storage_account: None,
                subscription_id: None,
            },
        }
    }

    pub fn with_storage_account(mut self, account: &str) -> Self {
        self.config.storage_account = Some(account.to_string());
        self
    }

    /// Get the configured storage account name
    pub fn get_storage_account(&self) -> Option<&str> {
        self.config.storage_account.as_deref()
    }

    /// Check if Azure CLI is installed and user is logged in
    pub async fn check_prerequisites(&self) -> Result<()> {
        // Check if az CLI is installed
        let output = AsyncCommand::new("az")
            .arg("--version")
            .output()
            .await
            .context("Azure CLI not found. Please install Azure CLI first.")?;

        if !output.status.success() {
            return Err(anyhow!("Azure CLI is not working properly"));
        }

        // Check if user is logged in
        let output = AsyncCommand::new("az")
            .args(["account", "show"])
            .output()
            .await
            .context("Failed to check Azure login status")?;

        if !output.status.success() {
            return Err(anyhow!(
                "Not logged in to Azure. Please run 'az login' first."
            ));
        }

        Ok(())
    }

    /// Get the default storage account from Azure CLI config
    #[allow(dead_code)]
    pub async fn get_default_storage_account(&self) -> Result<Option<String>> {
        let output = AsyncCommand::new("az")
            .args(["configure", "--list-defaults"])
            .output()
            .await
            .context("Failed to get Azure CLI defaults")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse the output to find storage account
            // This is a simplified parser - in reality you'd want more robust parsing
            for line in stdout.lines() {
                if line.contains("storage-account") {
                    if let Some(account) = line.split_whitespace().nth(1) {
                        return Ok(Some(account.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// List storage accounts in the current resource group or subscription
    pub async fn list_storage_accounts(&self) -> Result<Vec<StorageAccountInfo>> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args(["storage", "account", "list", "--output", "json"]);

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage account list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Azure CLI error: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let accounts: Vec<StorageAccountInfo> =
            serde_json::from_str(&stdout).context("Failed to parse storage account list JSON")?;

        Ok(accounts)
    }

    /// List containers in the storage account
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args(["storage", "container", "list", "--output", "json"]);

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage container list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Azure CLI error: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers: Vec<ContainerInfo> =
            serde_json::from_str(&stdout).context("Failed to parse container list JSON")?;

        Ok(containers)
    }

    /// List blobs in a container with optional prefix
    pub async fn list_blobs(&self, container: &str, prefix: Option<&str>) -> Result<Vec<BlobInfo>> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args([
            "storage",
            "blob",
            "list",
            "--container-name",
            container,
            "--output",
            "json",
        ]);

        if let Some(prefix_val) = prefix {
            cmd.args(["--prefix", prefix_val]);
        }

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage blob list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Azure CLI error: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let blobs: Vec<BlobInfo> =
            serde_json::from_str(&stdout).context("Failed to parse blob list JSON")?;

        Ok(blobs)
    }

    /// Upload a file to Azure storage
    #[allow(dead_code)]
    pub async fn upload_file(
        &self,
        local_path: &str,
        container: &str,
        blob_name: &str,
    ) -> Result<()> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args([
            "storage",
            "blob",
            "upload",
            "--file",
            local_path,
            "--container-name",
            container,
            "--name",
            blob_name,
            "--overwrite",
        ]);

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage blob upload")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Parse common errors and provide user-friendly messages
            if stderr.contains("Storage account") && stderr.contains("not found") {
                let account_name = self.config.storage_account.as_deref().unwrap_or("unknown");
                return Err(anyhow!(
                    "Storage account '{}' not found. Please verify the account name and ensure you have access to it.",
                    account_name
                ));
            } else if stderr.contains("container") && stderr.contains("not found") {
                return Err(anyhow!(
                    "Container '{}' not found. Please create the container first or verify the name.",
                    container
                ));
            } else if stderr.contains("resource name length is not within the permissible limits")
                || stderr.contains("OutOfRangeInput")
            {
                return Err(anyhow!(
                    "Invalid container name '{}'. Container names must be 3-63 characters long, lowercase letters, numbers, and hyphens only.",
                    container
                ));
            } else if stderr.contains("The specified container does not exist") {
                return Err(anyhow!(
                    "Container '{}' does not exist. Please create the container first.",
                    container
                ));
            } else if stderr.contains("does not have the required permissions") {
                return Err(anyhow!(
                    "Permission denied. You don't have the required permissions to upload to this storage account."
                ));
            } else if stderr.contains("AuthenticationFailed") {
                return Err(anyhow!(
                    "Authentication failed. Please verify your Azure credentials and permissions."
                ));
            }

            // For other errors, provide a simplified message
            return Err(anyhow!("Upload failed: {}", stderr.trim()));
        }

        Ok(())
    }

    /// Upload a directory to Azure storage using batch upload for better performance
    #[allow(dead_code)]
    pub async fn upload_batch(
        &self,
        local_dir: &str,
        container: &str,
        destination_path: Option<&str>,
        max_connections: u32,
    ) -> Result<()> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args([
            "storage",
            "blob",
            "upload-batch",
            "--source",
            local_dir,
            "--destination",
            container,
            "--overwrite",
            "--max-connections",
            &max_connections.to_string(),
        ]);

        if let Some(dest_path) = destination_path {
            cmd.args(["--destination-path", dest_path]);
        }

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage blob upload-batch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Parse common errors and provide user-friendly messages
            if stderr.contains("Storage account") && stderr.contains("not found") {
                let account_name = self.config.storage_account.as_deref().unwrap_or("unknown");
                return Err(anyhow!(
                    "Storage account '{}' not found. Please verify the account name and ensure you have access to it.",
                    account_name
                ));
            } else if stderr.contains("container") && stderr.contains("not found") {
                return Err(anyhow!(
                    "Container '{}' not found. Please create the container first or verify the name.",
                    container
                ));
            } else if stderr.contains("The specified container does not exist") {
                return Err(anyhow!(
                    "Container '{}' does not exist. Please create the container first.",
                    container
                ));
            } else if stderr.contains("does not have the required permissions") {
                return Err(anyhow!(
                    "Permission denied. You don't have the required permissions to upload to this storage account."
                ));
            } else if stderr.contains("AuthenticationFailed") {
                return Err(anyhow!(
                    "Authentication failed. Please verify your Azure credentials and permissions."
                ));
            }

            // For other errors, provide a simplified message
            return Err(anyhow!("Batch upload failed: {}", stderr.trim()));
        }

        Ok(())
    }

    /// Download a file from Azure storage
    #[allow(dead_code)]
    pub async fn download_file(
        &self,
        container: &str,
        blob_name: &str,
        local_path: &str,
    ) -> Result<()> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args([
            "storage",
            "blob",
            "download",
            "--container-name",
            container,
            "--name",
            blob_name,
            "--file",
            local_path,
        ]);

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage blob download")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Parse common errors and provide user-friendly messages
            if stderr.contains("Storage account") && stderr.contains("not found") {
                let account_name = self.config.storage_account.as_deref().unwrap_or("unknown");
                return Err(anyhow!(
                    "Storage account '{}' not found. Please verify the account name and ensure you have access to it.",
                    account_name
                ));
            } else if stderr.contains("container") && stderr.contains("not found") {
                return Err(anyhow!(
                    "Container '{}' not found. Please verify the container name.",
                    container
                ));
            } else if stderr.contains("resource name length is not within the permissible limits")
                || stderr.contains("OutOfRangeInput")
            {
                return Err(anyhow!(
                    "Invalid container name '{}'. Container names must be 3-63 characters long, lowercase letters, numbers, and hyphens only.",
                    container
                ));
            } else if stderr.contains("The specified container does not exist") {
                return Err(anyhow!("Container '{}' does not exist.", container));
            } else if stderr.contains("blob") && stderr.contains("not found") {
                return Err(anyhow!(
                    "Blob '{}' not found in container '{}'.",
                    blob_name,
                    container
                ));
            } else if stderr.contains("The specified blob does not exist") {
                return Err(anyhow!(
                    "Blob '{}' does not exist in container '{}'.",
                    blob_name,
                    container
                ));
            } else if stderr.contains("does not have the required permissions") {
                return Err(anyhow!(
                    "Permission denied. You don't have the required permissions to download from this storage account."
                ));
            } else if stderr.contains("AuthenticationFailed") {
                return Err(anyhow!(
                    "Authentication failed. Please verify your Azure credentials and permissions."
                ));
            }

            return Err(anyhow!("Download failed: {}", stderr.trim()));
        }

        Ok(())
    }

    /// Delete a blob from Azure storage
    #[allow(dead_code)]
    pub async fn delete_blob(&self, container: &str, blob_name: &str) -> Result<()> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args([
            "storage",
            "blob",
            "delete",
            "--container-name",
            container,
            "--name",
            blob_name,
        ]);

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage blob delete")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Parse common errors and provide user-friendly messages
            if stderr.contains("Storage account") && stderr.contains("not found") {
                let account_name = self.config.storage_account.as_deref().unwrap_or("unknown");
                return Err(anyhow!(
                    "Storage account '{}' not found. Please verify the account name and ensure you have access to it.",
                    account_name
                ));
            } else if stderr.contains("container") && stderr.contains("not found") {
                return Err(anyhow!(
                    "Container '{}' not found. Please verify the container name.",
                    container
                ));
            } else if stderr.contains("resource name length is not within the permissible limits")
                || stderr.contains("OutOfRangeInput")
            {
                return Err(anyhow!(
                    "Invalid container name '{}'. Container names must be 3-63 characters long, lowercase letters, numbers, and hyphens only.",
                    container
                ));
            } else if stderr.contains("The specified container does not exist") {
                return Err(anyhow!("Container '{}' does not exist.", container));
            } else if stderr.contains("does not have the required permissions") {
                return Err(anyhow!(
                    "Permission denied. You don't have the required permissions to delete from this storage account."
                ));
            } else if stderr.contains("AuthenticationFailed") {
                return Err(anyhow!(
                    "Authentication failed. Please verify your Azure credentials and permissions."
                ));
            }

            return Err(anyhow!("Delete failed: {}", stderr.trim()));
        }

        Ok(())
    }
}

// ============================================================================
// AzCopy Client - High-performance operations
// ============================================================================

/// Convert az:// URI to AzCopy-compatible HTTPS URL
/// Example: az://account/container/path -> https://account.blob.core.windows.net/container/path
pub fn convert_az_uri_to_url(az_uri: &str) -> Result<String> {
    if !az_uri.starts_with("az://") {
        return Err(anyhow!("Invalid Azure URI format. Expected az://..."));
    }

    let path = &az_uri[5..]; // Remove "az://"
    let parts: Vec<&str> = path.splitn(3, '/').collect();

    match parts.len() {
        0 | 1 => Err(anyhow!(
            "Invalid Azure URI '{}'. Expected format: az://account/container/[path]",
            az_uri
        )),
        2 => {
            // az://account/container
            Ok(format!(
                "https://{}.blob.core.windows.net/{}",
                parts[0], parts[1]
            ))
        }
        3 => {
            // az://account/container/path
            Ok(format!(
                "https://{}.blob.core.windows.net/{}/{}",
                parts[0], parts[1], parts[2]
            ))
        }
        _ => Err(anyhow!("Failed to parse Azure URI '{}'", az_uri)),
    }
}

#[derive(Clone)]
pub struct AzCopyClient {
    #[allow(dead_code)]
    config: AzureConfig,
}

impl AzCopyClient {
    pub fn new() -> Self {
        Self {
            config: AzureConfig {
                storage_account: None,
                subscription_id: None,
            },
        }
    }

    #[allow(dead_code)]
    pub fn with_storage_account(mut self, account: &str) -> Self {
        self.config.storage_account = Some(account.to_string());
        self
    }

    #[allow(dead_code)]
    pub fn get_storage_account(&self) -> Option<&str> {
        self.config.storage_account.as_deref()
    }

    /// Check if AzCopy is installed and Azure CLI is authenticated
    pub async fn check_prerequisites(&self) -> Result<()> {
        // Check if azcopy is installed
        let output = AsyncCommand::new("azcopy")
            .arg("--version")
            .output()
            .await
            .context(
                "AzCopy not found. Please install AzCopy from https://aka.ms/downloadazcopy",
            )?;

        if !output.status.success() {
            return Err(anyhow!("AzCopy is not working properly"));
        }

        // Check if user is logged in to Azure (azcopy uses Azure CLI credentials)
        let output = AsyncCommand::new("az")
            .args(["account", "show"])
            .output()
            .await
            .context("Failed to check Azure login status")?;

        if !output.status.success() {
            return Err(anyhow!(
                "Not logged in to Azure. Please run 'az login' first. AzCopy uses Azure CLI credentials."
            ));
        }

        Ok(())
    }

    /// Copy files/directories using AzCopy
    /// Supports local->azure, azure->local, and azure->azure
    #[allow(dead_code)]
    pub async fn copy(&self, source: &str, destination: &str, recursive: bool) -> Result<()> {
        let options = AzCopyOptions::new().with_recursive(recursive);
        self.copy_with_options(source, destination, &options).await
    }

    /// Copy files/directories using AzCopy with additional options
    pub async fn copy_with_options(
        &self,
        source: &str,
        destination: &str,
        options: &AzCopyOptions,
    ) -> Result<()> {
        let mut cmd = AsyncCommand::new("azcopy");
        cmd.args(["copy", source, destination]);

        // Apply common options
        options.apply_to_command(&mut cmd);

        // Use JSON output for better parsing
        cmd.args(["--output-type", "json"]);

        // IMPORTANT: Tell AzCopy to use Azure CLI credentials for authentication
        // This is set via environment variable
        cmd.env("AZCOPY_AUTO_LOGIN_TYPE", "AZCLI");

        // Capture stdout to parse JSON output
        // All azcopy output goes to stdout with --output-type json
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null()); // Discard stderr

        let mut child = cmd.spawn().context("Failed to execute azcopy copy")?;

        // Process stdout
        let failed_count = if let Some(stdout) = child.stdout.take() {
            crate::azcopy_output::handle_azcopy_output(stdout).await?
        } else {
            0
        };

        let status = child.wait().await.context("Failed to wait for azcopy")?;

        // Exit code 1 with failed transfers is expected - show warning but don't fail
        if !status.success() {
            if failed_count > 0 {
                // CompletedWithErrors - warning already shown, don't fail the operation
                return Ok(());
            } else {
                // Actual failure
                return Err(anyhow!(
                    "AzCopy operation failed with exit code: {}",
                    status.code().unwrap_or(-1)
                ));
            }
        }

        Ok(())
    }

    /// Sync directories using AzCopy (rsync-like functionality)
    #[allow(dead_code)]
    pub async fn sync(
        &self,
        source: &str,
        destination: &str,
        delete_destination: bool,
    ) -> Result<()> {
        let options = AzCopyOptions::new();
        self.sync_with_options(source, destination, delete_destination, &options)
            .await
    }

    /// Sync directories using AzCopy with additional options
    pub async fn sync_with_options(
        &self,
        source: &str,
        destination: &str,
        delete_destination: bool,
        options: &AzCopyOptions,
    ) -> Result<()> {
        let mut cmd = AsyncCommand::new("azcopy");
        cmd.args(["sync", source, destination]);

        if delete_destination {
            cmd.arg("--delete-destination=true");
        }

        // Apply common options (excluding recursive as sync is always recursive)
        if options.dry_run {
            cmd.arg("--dry-run");
        }

        if let Some(mbps) = options.cap_mbps {
            cmd.arg(format!("--cap-mbps={}", mbps));
        }

        if let Some(pattern) = &options.include_pattern {
            cmd.arg(format!("--include-pattern={}", pattern));
        }

        if let Some(pattern) = &options.exclude_pattern {
            cmd.arg(format!("--exclude-pattern={}", pattern));
        }

        // Use Azure CLI credentials
        cmd.env("AZCOPY_AUTO_LOGIN_TYPE", "AZCLI");

        // Inherit stdout/stderr so user sees real-time progress
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        let status = cmd
            .status()
            .await
            .context("Failed to execute azcopy sync")?;

        if !status.success() {
            return Err(anyhow!(
                "AzCopy sync operation failed with exit code: {}",
                status.code().unwrap_or(-1)
            ));
        }

        Ok(())
    }

    /// Remove files/directories using AzCopy
    #[allow(dead_code)]
    pub async fn remove(&self, target: &str, recursive: bool) -> Result<()> {
        let options = AzCopyOptions::new().with_recursive(recursive);
        self.remove_with_options(target, &options).await
    }

    /// Remove files/directories using AzCopy with additional options
    pub async fn remove_with_options(&self, target: &str, options: &AzCopyOptions) -> Result<()> {
        let mut cmd = AsyncCommand::new("azcopy");
        cmd.args(["remove", target]);

        // Apply common options
        options.apply_to_command(&mut cmd);

        // Use JSON output for better parsing
        cmd.args(["--output-type", "json"]);

        // Use Azure CLI credentials
        cmd.env("AZCOPY_AUTO_LOGIN_TYPE", "AZCLI");

        // Capture stdout to parse JSON output
        // All azcopy output goes to stdout with --output-type json
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null()); // Discard stderr

        let mut child = cmd.spawn().context("Failed to execute azcopy remove")?;

        // Process stdout
        let failed_count = if let Some(stdout) = child.stdout.take() {
            crate::azcopy_output::handle_azcopy_output_with_operation(
                stdout,
                crate::azcopy_output::AzCopyOperation::Remove,
            )
            .await?
        } else {
            0
        };

        let status = child.wait().await.context("Failed to wait for azcopy")?;

        // Exit code 1 with failed transfers is expected - show warning but don't fail
        if !status.success() {
            if failed_count > 0 {
                // CompletedWithErrors - warning already shown, don't fail the operation
                return Ok(());
            } else {
                // Actual failure
                return Err(anyhow!(
                    "AzCopy remove operation failed with exit code: {}",
                    status.code().unwrap_or(-1)
                ));
            }
        }

        Ok(())
    }

    /// Parse AzCopy errors and provide user-friendly messages
    #[allow(dead_code)]
    fn parse_azcopy_error(&self, stderr: &str) -> anyhow::Error {
        if stderr.contains("authentication") || stderr.contains("AuthenticationFailed") {
            anyhow!(
                "Authentication failed. Please verify your Azure credentials by running 'az login'."
            )
        } else if stderr.contains("AuthorizationPermissionMismatch") {
            anyhow!(
                "Permission denied. Your Azure account doesn't have permission to write to this storage account.\n\
                \n\
                To fix this, you need one of these roles assigned:\n\
                  - Storage Blob Data Contributor\n\
                  - Storage Blob Data Owner\n\
                \n\
                Ask your Azure administrator to grant these permissions, or use:\n\
                  az role assignment create --role \"Storage Blob Data Contributor\" \\\n\
                    --assignee <your-email> \\\n\
                    --scope /subscriptions/<subscription-id>/resourceGroups/<rg>/providers/Microsoft.Storage/storageAccounts/<account>"
            )
        } else if stderr.contains("BlobNotFound") || stderr.contains("not found") {
            anyhow!("Resource not found. Please verify the path and container name.")
        } else if stderr.contains("ContainerNotFound") {
            anyhow!("Container not found. Please create the container first or verify the name.")
        } else if stderr.contains("AccountNotFound") {
            anyhow!("Storage account not found. Please verify the account name and ensure you have access to it.")
        } else {
            // Return the actual error for debugging
            anyhow!("AzCopy operation failed: {}", stderr.trim())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_client_new() {
        let client = AzureClient::new();
        assert!(client.config.storage_account.is_none());
        assert!(client.config.subscription_id.is_none());
    }

    #[test]
    fn test_azure_client_with_storage_account() {
        let client = AzureClient::new().with_storage_account("myaccount");
        assert_eq!(client.config.storage_account, Some("myaccount".to_string()));
    }

    #[test]
    fn test_azure_client_builder_pattern() {
        let client = AzureClient::new()
            .with_storage_account("testaccount")
            .with_storage_account("newaccount");
        assert_eq!(
            client.config.storage_account,
            Some("newaccount".to_string())
        );
    }

    #[test]
    fn test_blob_info_deserialization() {
        let json = r#"{
            "name": "test.txt",
            "properties": {
                "contentLength": 1024,
                "lastModified": "2024-01-01T00:00:00Z",
                "contentType": "text/plain"
            }
        }"#;

        let blob: BlobInfo = serde_json::from_str(json).unwrap();
        assert_eq!(blob.name, "test.txt");
        assert_eq!(blob.properties.content_length, 1024);
        assert_eq!(blob.properties.last_modified, "2024-01-01T00:00:00Z");
        assert_eq!(blob.properties.content_type, Some("text/plain".to_string()));
    }

    #[test]
    fn test_blob_info_deserialization_no_content_type() {
        let json = r#"{
            "name": "unknown.bin",
            "properties": {
                "contentLength": 2048,
                "lastModified": "2024-01-02T00:00:00Z"
            }
        }"#;

        let blob: BlobInfo = serde_json::from_str(json).unwrap();
        assert_eq!(blob.name, "unknown.bin");
        assert_eq!(blob.properties.content_length, 2048);
        assert_eq!(blob.properties.content_type, None);
    }

    #[test]
    fn test_container_info_deserialization() {
        let json = r#"{
            "name": "mycontainer",
            "properties": {
                "lastModified": "2024-01-01T00:00:00Z",
                "publicAccess": "container"
            }
        }"#;

        let container: ContainerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(container.name, "mycontainer");
        assert_eq!(container.properties.last_modified, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_container_list_deserialization() {
        let json = r#"[
            {
                "name": "container1",
                "properties": {
                    "lastModified": "2024-01-01T00:00:00Z"
                }
            },
            {
                "name": "container2",
                "properties": {
                    "lastModified": "2024-01-02T00:00:00Z"
                }
            }
        ]"#;

        let containers: Vec<ContainerInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].name, "container1");
        assert_eq!(containers[1].name, "container2");
    }

    #[test]
    fn test_blob_list_deserialization() {
        let json = r#"[
            {
                "name": "file1.txt",
                "properties": {
                    "contentLength": 100,
                    "lastModified": "2024-01-01T00:00:00Z",
                    "contentType": "text/plain"
                }
            },
            {
                "name": "dir/file2.txt",
                "properties": {
                    "contentLength": 200,
                    "lastModified": "2024-01-02T00:00:00Z",
                    "contentType": "text/plain"
                }
            }
        ]"#;

        let blobs: Vec<BlobInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(blobs.len(), 2);
        assert_eq!(blobs[0].name, "file1.txt");
        assert_eq!(blobs[0].properties.content_length, 100);
        assert_eq!(blobs[1].name, "dir/file2.txt");
        assert_eq!(blobs[1].properties.content_length, 200);
    }

    #[test]
    fn test_storage_account_info_deserialization() {
        let json = r#"{
            "name": "mystorageaccount",
            "location": "eastus2",
            "resourceGroup": "my-resource-group",
            "creationTime": "2024-01-01T00:00:00.000000+00:00"
        }"#;

        let account: StorageAccountInfo = serde_json::from_str(json).unwrap();
        assert_eq!(account.name, "mystorageaccount");
        assert_eq!(account.location, "eastus2");
        assert_eq!(account.resource_group, "my-resource-group");
        assert_eq!(account.creation_time, "2024-01-01T00:00:00.000000+00:00");
    }

    #[test]
    fn test_storage_account_list_deserialization() {
        let json = r#"[
            {
                "name": "account1",
                "location": "eastus",
                "resourceGroup": "rg1",
                "creationTime": "2024-01-01T00:00:00.000000+00:00"
            },
            {
                "name": "account2",
                "location": "westus",
                "resourceGroup": "rg2",
                "creationTime": "2024-01-02T00:00:00.000000+00:00"
            }
        ]"#;

        let accounts: Vec<StorageAccountInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].name, "account1");
        assert_eq!(accounts[0].location, "eastus");
        assert_eq!(accounts[0].resource_group, "rg1");
        assert_eq!(accounts[1].name, "account2");
        assert_eq!(accounts[1].location, "westus");
        assert_eq!(accounts[1].resource_group, "rg2");
    }
}
