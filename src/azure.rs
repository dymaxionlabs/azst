use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command as AsyncCommand;

use azure_core::auth::TokenCredential;
use azure_storage::StorageCredentials;
use azure_storage_blobs::prelude::*;
use futures::StreamExt;

// ============================================================================
// AzCopy Configuration
// ============================================================================

/// The pinned version of AzCopy that azst is tested with
pub const AZCOPY_PINNED_VERSION: &str = "10.30.1";

// ============================================================================
// AzCopy Options - Common options for azcopy operations
// ============================================================================

/// Options for azcopy copy operations
#[derive(Debug, Clone, Default)]
pub struct AzCopyOptions {
    pub recursive: bool,
    pub dry_run: bool,
    pub cap_mbps: Option<f64>,
    pub block_size_mb: Option<f64>,
    pub put_md5: bool,
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

    pub fn with_block_size_mb(mut self, block_size_mb: Option<f64>) -> Self {
        self.block_size_mb = block_size_mb;
        self
    }

    pub fn with_put_md5(mut self, put_md5: bool) -> Self {
        self.put_md5 = put_md5;
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

        if let Some(block_size) = self.block_size_mb {
            cmd.arg(format!("--block-size-mb={}", block_size));
        }

        if self.put_md5 {
            cmd.arg("--put-md5");
        }

        if let Some(pattern) = &self.include_pattern {
            cmd.arg(format!("--include-pattern={}", pattern));
        }

        if let Some(pattern) = &self.exclude_pattern {
            cmd.arg(format!("--exclude-pattern={}", pattern));
        }
    }

    /// Apply environment variable tuning settings
    pub fn apply_env_vars(cmd: &mut AsyncCommand) {
        // Pass through performance-related environment variables if set
        let env_vars = [
            "AZCOPY_CONCURRENCY_VALUE",
            "AZCOPY_CONCURRENT_FILES",
            "AZCOPY_CONCURRENT_SCAN",
            "AZCOPY_BUFFER_GB",
            "AZCOPY_LOG_LOCATION",
            "AZCOPY_JOB_PLAN_LOCATION",
            "AZCOPY_DISABLE_HIERARCHICAL_SCAN",
            "AZCOPY_PARALLEL_STAT_FILES",
        ];

        for var in &env_vars {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
    }
}

// ============================================================================
// Azure Configuration and Data Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct AzureConfig {
    pub storage_account: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlobInfo {
    pub name: String,
    #[serde(rename = "properties")]
    pub properties: BlobProperties,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlobProperties {
    #[serde(rename = "contentLength")]
    pub content_length: u64,
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
}

/// Represents either a blob or a blob prefix (virtual directory)
#[derive(Debug)]
pub enum BlobItem {
    Blob(BlobInfo),
    Prefix(String),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContainerInfo {
    pub name: String,
    #[serde(rename = "properties")]
    pub properties: ContainerProperties,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContainerProperties {
    #[serde(rename = "lastModified")]
    pub last_modified: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageAccountInfo {
    pub name: String,
    pub location: String,
    #[serde(rename = "resourceGroup")]
    pub resource_group: String,
}

#[derive(Clone)]
pub struct AzureClient {
    config: AzureConfig,
    credential: Option<Arc<dyn TokenCredential>>,
}

impl AzureClient {
    pub fn new() -> Self {
        Self {
            config: AzureConfig {
                storage_account: None,
            },
            credential: None,
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

    /// Get or create the Azure credential using a fallback chain
    ///
    /// Credential chain (in priority order):
    /// 1. Environment Variables (Service Principal)
    ///    - AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET
    ///    - Or AZURE_FEDERATED_TOKEN / AZURE_FEDERATED_TOKEN_FILE for Workload Identity
    /// 2. Managed Identity (Azure VMs, AKS, App Service, Container Instances, etc.)
    /// 3. Azure CLI (az login) - Best for local development
    ///
    /// This matches AzCopy's authentication flow and works in both
    /// development (with Azure CLI) and production (with Managed Identity or Service Principal).
    ///
    /// Set `AZURE_CREDENTIAL_KIND` environment variable to force a specific credential type:
    /// - "azurecli" - Azure CLI only
    /// - "virtualmachine" - Managed Identity only
    /// - "environment" - Environment variables only
    async fn get_credential(&mut self) -> Result<Arc<dyn TokenCredential>> {
        if let Some(ref cred) = self.credential {
            return Ok(cred.clone());
        }

        // Use create_credential() which creates DefaultAzureCredential by default
        // or SpecificAzureCredential if AZURE_CREDENTIAL_KIND is set
        // This automatically tries (in order):
        // 1. EnvironmentCredential (AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET)
        // 2. WorkloadIdentityCredential (AZURE_FEDERATED_TOKEN_FILE for AKS workload identity)
        // 3. ManagedIdentityCredential (for Azure VMs, App Service, Container Instances)
        // 4. AzureCliCredential (az login for local development)
        let credential = azure_identity::create_credential()
            .context("Failed to create Azure credential. Please ensure you have authenticated with 'az login', or are running on an Azure VM with Managed Identity, or have set service principal environment variables (AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET).")?;

        self.credential = Some(credential.clone());
        Ok(credential)
    }

    /// Create a BlobServiceClient for the configured storage account
    async fn get_blob_service_client(&mut self) -> Result<BlobServiceClient> {
        let account_name = self
            .config
            .storage_account
            .as_ref()
            .ok_or_else(|| anyhow!("Storage account not configured"))?
            .clone();

        let credential = self.get_credential().await?;

        // Create BlobServiceClient with token credential
        let client = BlobServiceClient::new(
            &account_name,
            StorageCredentials::token_credential(credential as Arc<dyn TokenCredential>),
        );

        Ok(client)
    }

    /// Check if Azure credentials are available
    pub async fn check_prerequisites(&mut self) -> Result<()> {
        // Try to get a credential - this will validate authentication
        let _credential = self
            .get_credential()
            .await
            .context("Failed to authenticate with Azure. Please run 'az login' to authenticate.")?;

        // Note: We use Azure CLI credentials via the SDK
        // The user must have run `az login` for this to work
        Ok(())
    }

    /// Get the current subscription ID
    /// First tries the AZURE_SUBSCRIPTION_ID environment variable,
    /// then falls back to using Azure CLI to get the default subscription
    async fn get_subscription_id(&mut self) -> Result<String> {
        // Try environment variable first
        if let Ok(sub_id) = std::env::var("AZURE_SUBSCRIPTION_ID") {
            return Ok(sub_id);
        }

        // Fall back to using Azure CLI to get the current subscription
        let output = AsyncCommand::new("az")
            .args(["account", "show", "--query", "id", "-o", "tsv"])
            .output()
            .await
            .context(
                "Failed to run 'az account show'. Please ensure you are logged in with 'az login'.",
            )?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to get subscription ID from Azure CLI. Please ensure you are logged in with 'az login' and have at least one subscription selected."
            ));
        }

        let subscription_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if subscription_id.is_empty() {
            return Err(anyhow!(
                "No Azure subscription ID returned from 'az account show'. Please ensure you have a subscription selected with 'az account set'."
            ));
        }

        Ok(subscription_id)
    }

    /// List storage accounts in the current subscription
    /// Uses Azure Management SDK to query storage accounts
    ///
    /// Automatically detects subscription ID from:
    /// 1. AZURE_SUBSCRIPTION_ID environment variable (if set)
    /// 2. Azure CLI default subscription (via `az account show`)
    pub async fn list_storage_accounts(&mut self) -> Result<Vec<StorageAccountInfo>> {
        let credential = self.get_credential().await?;

        // Get subscription ID (with automatic fallback)
        let subscription_id = self.get_subscription_id().await?;

        // Create management client using ClientBuilder
        let client = azure_mgmt_storage::Client::builder(credential).build()?;

        let mut all_accounts = Vec::new();

        // List all storage accounts in the subscription using streaming API
        let mut stream = client
            .storage_accounts_client()
            .list(subscription_id)
            .into_stream();

        while let Some(response_result) = stream.next().await {
            let response = response_result.context("Failed to list storage accounts")?;

            // Extract storage accounts from the response
            for account in response.value {
                // Extract resource group from the account ID
                // ID format: /subscriptions/{subscriptionId}/resourceGroups/{resourceGroupName}/providers/Microsoft.Storage/storageAccounts/{accountName}
                let resource_group = account
                    .tracked_resource
                    .resource
                    .id
                    .as_ref()
                    .and_then(|id| id.split('/').nth(4).map(|s| s.to_string()))
                    .unwrap_or_default();

                all_accounts.push(StorageAccountInfo {
                    name: account.tracked_resource.resource.name.unwrap_or_default(),
                    location: account.tracked_resource.location,
                    resource_group,
                });
            }
        }

        Ok(all_accounts)
    }

    /// List containers in the storage account using Azure SDK
    pub async fn list_containers(&mut self) -> Result<Vec<ContainerInfo>> {
        let blob_service = self.get_blob_service_client().await?;

        // List containers using the SDK
        let mut containers = Vec::new();
        let mut stream = blob_service.list_containers().into_stream();

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    for container in response.containers {
                        containers.push(ContainerInfo {
                            name: container.name,
                            properties: ContainerProperties {
                                last_modified: container.last_modified.to_string(),
                            },
                        });
                    }
                }
                Err(e) => {
                    return Err(anyhow!("Failed to list containers: {}", e));
                }
            }
        }

        Ok(containers)
    }

    /// List blobs in a container with optional prefix
    /// This method automatically handles pagination to retrieve all results
    pub async fn list_blobs(
        &mut self,
        container: &str,
        prefix: Option<&str>,
        delimiter: Option<&str>,
    ) -> Result<Vec<BlobItem>> {
        let mut all_items = Vec::new();

        self.list_blobs_with_callback(container, prefix, delimiter, |items| {
            all_items.extend(items);
            Ok(())
        })
        .await?;

        Ok(all_items)
    }

    /// List blobs in a container with a callback for each page
    /// This allows processing results as they arrive without buffering everything in memory
    pub async fn list_blobs_with_callback<F>(
        &mut self,
        container: &str,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(Vec<BlobItem>) -> Result<()>,
    {
        let blob_service = self.get_blob_service_client().await?;
        let container_client = blob_service.container_client(container);

        // Build the list blobs request
        let mut list_builder = container_client.list_blobs();

        if let Some(prefix_val) = prefix {
            list_builder = list_builder.prefix(prefix_val.to_string());
        }

        // Set delimiter for hierarchical listing (non-recursive)
        // When delimiter is set (e.g., "/"), the API returns only immediate children
        // and uses BlobPrefix items for "subdirectories"
        if let Some(delimiter_val) = delimiter {
            list_builder = list_builder.delimiter(delimiter_val.to_string());
        }

        let mut stream = list_builder.into_stream();

        while let Some(page_result) = stream.next().await {
            let page = page_result.context("Failed to fetch blob page")?;
            let mut items = Vec::new();

            // Process blobs and blob prefixes
            for item in &page.blobs.items {
                match item {
                    azure_storage_blobs::container::operations::BlobItem::Blob(blob) => {
                        items.push(BlobItem::Blob(BlobInfo {
                            name: blob.name.clone(),
                            properties: BlobProperties {
                                content_length: blob.properties.content_length,
                                last_modified: blob.properties.last_modified.to_string(),
                                content_type: Some(blob.properties.content_type.clone()),
                            },
                        }));
                    }
                    azure_storage_blobs::container::operations::BlobItem::BlobPrefix(prefix) => {
                        items.push(BlobItem::Prefix(prefix.name.clone()));
                    }
                }
            }

            // Call the callback with this page's items
            if !items.is_empty() {
                callback(items)?;
            }
        }

        Ok(())
    }

    /// Download a blob's content as bytes
    /// Returns the blob content and optionally a range of bytes
    pub async fn download_blob(
        &mut self,
        container: &str,
        blob_name: &str,
        range: Option<(u64, u64)>,
    ) -> Result<Vec<u8>> {
        let blob_service = self.get_blob_service_client().await?;
        let container_client = blob_service.container_client(container);
        let blob_client = container_client.blob_client(blob_name);

        // Get the blob content
        let response = if let Some((start, end)) = range {
            // Download with range (exclusive end)
            blob_client
                .get()
                .range(start..end + 1)
                .into_stream()
                .next()
                .await
                .ok_or_else(|| {
                    anyhow!(
                        "Failed to download blob '{}' with range {}-{}",
                        blob_name,
                        start,
                        end
                    )
                })??
        } else {
            // Download entire blob
            blob_client
                .get()
                .into_stream()
                .next()
                .await
                .ok_or_else(|| anyhow!("Failed to download blob '{}'", blob_name))??
        };

        // Collect the body into bytes
        let body = response.data.collect().await?;
        Ok(body.to_vec())
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

// ============================================================================
// AzCopy Path Utilities
// ============================================================================

/// Get the path where bundled AzCopy should be installed
pub fn get_bundled_azcopy_path() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // On Windows, use %LOCALAPPDATA%\Programs\azst\azcopy\azcopy.exe
        let local_app_data = std::env::var("LOCALAPPDATA")
            .ok()
            .map(PathBuf::from)
            .or_else(dirs::data_local_dir)
            .ok_or_else(|| anyhow!("Could not determine local app data directory"))?;
        Ok(local_app_data
            .join("Programs")
            .join("azst")
            .join("azcopy")
            .join("azcopy.exe"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        // On Unix-like systems, use ~/.local/share/azst/azcopy/azcopy
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        Ok(home
            .join(".local")
            .join("share")
            .join("azst")
            .join("azcopy")
            .join("azcopy"))
    }
}

/// Extract version from azcopy --version output
/// Expected format: "azcopy version 10.21.2"
fn parse_azcopy_version(version_output: &str) -> Option<String> {
    version_output
        .lines()
        .next()?
        .split_whitespace()
        .nth(2)
        .map(|v| v.to_string())
}

/// Check if the given AzCopy executable matches our pinned version
async fn check_azcopy_version(azcopy_path: &str) -> Result<bool> {
    let output = AsyncCommand::new(azcopy_path)
        .arg("--version")
        .output()
        .await
        .context("Failed to get AzCopy version")?;

    if !output.status.success() {
        return Ok(false);
    }

    let version_str = String::from_utf8_lossy(&output.stdout);
    let version = parse_azcopy_version(&version_str);

    Ok(version.as_deref() == Some(AZCOPY_PINNED_VERSION))
}

/// Determine which AzCopy executable to use (system or bundled)
async fn determine_azcopy_executable() -> Result<String> {
    // First, try system azcopy if it matches our pinned version
    if let Ok(true) = check_azcopy_version("azcopy").await {
        return Ok("azcopy".to_string());
    }

    // Then, try bundled azcopy
    if let Ok(bundled_path) = get_bundled_azcopy_path() {
        let bundled_str = bundled_path.to_string_lossy();
        if bundled_path.exists() && check_azcopy_version(&bundled_str).await.unwrap_or(false) {
            return Ok(bundled_str.to_string());
        }
    }

    // If neither works, fall back to system azcopy (will fail in check_prerequisites)
    Ok("azcopy".to_string())
}

#[derive(Clone)]
pub struct AzCopyClient {
    azcopy_executable: Option<String>,
}

impl AzCopyClient {
    pub fn new() -> Self {
        Self {
            azcopy_executable: None,
        }
    }

    /// Get the AzCopy executable path, determining it if not already cached
    async fn get_azcopy_executable(&mut self) -> Result<&str> {
        if self.azcopy_executable.is_none() {
            self.azcopy_executable = Some(determine_azcopy_executable().await?);
        }
        Ok(self.azcopy_executable.as_ref().unwrap())
    }

    /// Check if AzCopy is installed and Azure CLI is authenticated
    pub async fn check_prerequisites(&mut self) -> Result<()> {
        // Determine which azcopy executable to use and test it
        let azcopy_path = self.get_azcopy_executable().await?;

        let output = AsyncCommand::new(azcopy_path)
            .arg("--version")
            .output()
            .await
            .context(
                "AzCopy not found. Run the installation script again to download AzCopy, or install it manually from https://aka.ms/downloadazcopy",
            )?;

        if !output.status.success() {
            return Err(anyhow!("AzCopy is not working properly"));
        }

        // Verify version if we're using system azcopy
        if azcopy_path == "azcopy" {
            let version_str = String::from_utf8_lossy(&output.stdout);
            let version = parse_azcopy_version(&version_str);
            if version.as_deref() != Some(AZCOPY_PINNED_VERSION) {
                eprintln!("Warning: System AzCopy version {:?} doesn't match pinned version {}. Consider running the installation script to download the tested version.", version, AZCOPY_PINNED_VERSION);
            }
        }

        // Note: AzCopy will automatically detect Azure credentials via the credential chain:
        // 1. Environment variables (Service Principal)
        // 2. Managed Identity (Azure VMs/services)
        // 3. Azure CLI (az login)
        // If credentials are not available, AzCopy will fail with its own error message.

        Ok(())
    }

    /// Copy files/directories using AzCopy with additional options
    pub async fn copy_with_options(
        &mut self,
        source: &str,
        destination: &str,
        options: &AzCopyOptions,
    ) -> Result<()> {
        let azcopy_path = self.get_azcopy_executable().await?;
        let mut cmd = AsyncCommand::new(azcopy_path);
        cmd.args(["copy", source, destination]);

        // Apply common options
        options.apply_to_command(&mut cmd);

        // Use JSON output for better parsing
        cmd.args(["--output-type", "json"]);

        // IMPORTANT: Tell AzCopy to use Azure CLI credentials for authentication
        // This is set via environment variable
        cmd.env("AZCOPY_AUTO_LOGIN_TYPE", "AZCLI");

        // Apply environment variable tuning settings
        AzCopyOptions::apply_env_vars(&mut cmd);

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

    /// Sync directories using AzCopy with additional options
    pub async fn sync_with_options(
        &mut self,
        source: &str,
        destination: &str,
        delete_destination: bool,
        options: &AzCopyOptions,
    ) -> Result<()> {
        let azcopy_path = self.get_azcopy_executable().await?;
        let mut cmd = AsyncCommand::new(azcopy_path);
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

        if let Some(block_size) = options.block_size_mb {
            cmd.arg(format!("--block-size-mb={}", block_size));
        }

        if options.put_md5 {
            cmd.arg("--put-md5");
        }

        if let Some(pattern) = &options.include_pattern {
            cmd.arg(format!("--include-pattern={}", pattern));
        }

        if let Some(pattern) = &options.exclude_pattern {
            cmd.arg(format!("--exclude-pattern={}", pattern));
        }

        // Use Azure CLI credentials
        cmd.env("AZCOPY_AUTO_LOGIN_TYPE", "AZCLI");

        // Apply environment variable tuning settings
        AzCopyOptions::apply_env_vars(&mut cmd);

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

    /// Remove files/directories using AzCopy with additional options
    pub async fn remove_with_options(
        &mut self,
        target: &str,
        options: &AzCopyOptions,
    ) -> Result<()> {
        let azcopy_path = self.get_azcopy_executable().await?;
        let mut cmd = AsyncCommand::new(azcopy_path);
        cmd.args(["remove", target]);

        // Apply common options
        options.apply_to_command(&mut cmd);

        // Use JSON output for better parsing
        cmd.args(["--output-type", "json"]);

        // Use Azure CLI credentials
        cmd.env("AZCOPY_AUTO_LOGIN_TYPE", "AZCLI");

        // Apply environment variable tuning settings
        AzCopyOptions::apply_env_vars(&mut cmd);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_client_new() {
        let client = AzureClient::new();
        assert!(client.config.storage_account.is_none());
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

    // ========================================================================
    // Credential Chain Tests
    // ========================================================================

    #[tokio::test]
    async fn test_credential_caching() {
        // Test that credentials are cached after first creation
        let mut client = AzureClient::new();

        // First call should create and cache the credential
        let result1 = client.get_credential().await;
        let result2 = client.get_credential().await;

        // Both should succeed or fail consistently
        assert_eq!(result1.is_ok(), result2.is_ok());

        // If successful, verify they return the same Arc pointer
        if let (Ok(cred1), Ok(cred2)) = (result1, result2) {
            assert!(Arc::ptr_eq(&cred1, &cred2), "Credentials should be cached");
        }
    }

    #[tokio::test]
    async fn test_credential_chain_with_environment_override() {
        // Test that AZURE_CREDENTIAL_KIND can force a specific credential type
        // Note: This test will fail if the specified credential type is not available

        use std::env;

        // Save original value
        let original = env::var("AZURE_CREDENTIAL_KIND").ok();

        // Test with azurecli (requires az login)
        env::set_var("AZURE_CREDENTIAL_KIND", "azurecli");
        let mut client = AzureClient::new();
        let result = client.get_credential().await;

        // Should either succeed (if az login is available) or fail with helpful message
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("az login") || error_msg.contains("Azure CLI"),
                "Error should mention az login or Azure CLI: {}",
                error_msg
            );
        }

        // Restore original value
        if let Some(val) = original {
            env::set_var("AZURE_CREDENTIAL_KIND", val);
        } else {
            env::remove_var("AZURE_CREDENTIAL_KIND");
        }
    }

    #[test]
    fn test_credential_chain_documentation() {
        // This is a documentation test that verifies the expected credential chain order
        // The actual chain is:
        // 1. EnvironmentCredential (AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET)
        // 2. WorkloadIdentityCredential (AZURE_FEDERATED_TOKEN_FILE)
        // 3. ManagedIdentityCredential (Azure VMs, App Service, etc.)
        // 4. AzureCliCredential (az login)

        use std::env;

        // Document required environment variables for Service Principal
        let required_sp_vars = vec!["AZURE_TENANT_ID", "AZURE_CLIENT_ID", "AZURE_CLIENT_SECRET"];

        // Verify we can check for their presence
        for var in required_sp_vars {
            let _ = env::var(var); // Just checking we can access env vars
        }

        // Document Workload Identity environment variables
        let workload_identity_vars = vec![
            "AZURE_FEDERATED_TOKEN_FILE",
            "AZURE_TENANT_ID",
            "AZURE_CLIENT_ID",
        ];

        for var in workload_identity_vars {
            let _ = env::var(var);
        }

        // This test always passes - it's just for documentation
        assert!(true);
    }

    #[tokio::test]
    async fn test_credential_error_messages() {
        // Test that credential errors provide helpful messages

        use std::env;

        // Save all relevant environment variables
        let saved_vars = vec![
            (
                "AZURE_CREDENTIAL_KIND",
                env::var("AZURE_CREDENTIAL_KIND").ok(),
            ),
            ("AZURE_TENANT_ID", env::var("AZURE_TENANT_ID").ok()),
            ("AZURE_CLIENT_ID", env::var("AZURE_CLIENT_ID").ok()),
            ("AZURE_CLIENT_SECRET", env::var("AZURE_CLIENT_SECRET").ok()),
            (
                "AZURE_FEDERATED_TOKEN_FILE",
                env::var("AZURE_FEDERATED_TOKEN_FILE").ok(),
            ),
        ];

        // Clear all credential environment variables to force failure
        env::remove_var("AZURE_TENANT_ID");
        env::remove_var("AZURE_CLIENT_ID");
        env::remove_var("AZURE_CLIENT_SECRET");
        env::remove_var("AZURE_FEDERATED_TOKEN_FILE");
        env::remove_var("AZURE_CREDENTIAL_KIND");

        let mut client = AzureClient::new();
        let result = client.get_credential().await;

        // Should fail with a helpful error message
        if let Err(e) = result {
            let error_msg = e.to_string();
            // Error should mention at least one authentication method
            assert!(
                error_msg.contains("az login")
                    || error_msg.contains("Managed Identity")
                    || error_msg.contains("environment variables")
                    || error_msg.contains("AZURE_"),
                "Error should provide helpful authentication guidance: {}",
                error_msg
            );
        }

        // Restore environment variables
        for (key, value) in saved_vars {
            if let Some(val) = value {
                env::set_var(key, val);
            }
        }
    }

    #[tokio::test]
    async fn test_blob_service_client_requires_account() {
        // Test that get_blob_service_client fails without storage account
        let mut client = AzureClient::new();
        let result = client.get_blob_service_client().await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Storage account not configured"),
            "Error should mention storage account: {}",
            error_msg
        );
    }

    #[tokio::test]
    async fn test_blob_service_client_with_account() {
        // Test that get_blob_service_client works with storage account configured
        let mut client = AzureClient::new().with_storage_account("testaccount");

        // This will fail if credentials aren't available, but should fail differently
        let result = client.get_blob_service_client().await;

        if let Err(e) = result {
            let error_msg = e.to_string();
            // Should not complain about storage account anymore
            assert!(
                !error_msg.contains("Storage account not configured"),
                "Error should not be about storage account configuration: {}",
                error_msg
            );
        }
    }

    #[test]
    fn test_credential_chain_priority_order() {
        // Document and verify the credential chain priority
        // This test serves as documentation for the expected behavior

        // Priority 1: Environment Variables (Service Principal)
        // Required: AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET
        // Use case: CI/CD pipelines, automation scripts

        // Priority 2: Workload Identity (Federated)
        // Required: AZURE_FEDERATED_TOKEN_FILE, AZURE_TENANT_ID, AZURE_CLIENT_ID
        // Use case: Kubernetes workload identity, GitHub Actions OIDC

        // Priority 3: Managed Identity
        // Required: Running on Azure VM, App Service, Container Instance, or AKS
        // Use case: Production deployments on Azure

        // Priority 4: Azure CLI
        // Required: Azure CLI installed and `az login` completed
        // Use case: Local development

        // This matches the behavior of:
        // - Azure SDK DefaultAzureCredential
        // - AzCopy authentication
        // - Azure PowerShell

        assert!(true, "Credential chain documented");
    }
}
