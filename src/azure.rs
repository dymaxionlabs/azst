use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::process::Command as AsyncCommand;

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
            return Err(anyhow!("Upload failed: {}", stderr));
        }

        Ok(())
    }

    /// Download a file from Azure storage
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
            return Err(anyhow!("Download failed: {}", stderr));
        }

        Ok(())
    }

    /// Delete a blob from Azure storage
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
            return Err(anyhow!("Delete failed: {}", stderr));
        }

        Ok(())
    }

    /// Create a container
    pub async fn create_container(&self, container: &str) -> Result<()> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args(["storage", "container", "create", "--name", container]);

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage container create")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Container creation failed: {}", stderr));
        }

        Ok(())
    }

    /// Delete a container
    pub async fn delete_container(&self, container: &str) -> Result<()> {
        let mut cmd = AsyncCommand::new("az");
        cmd.args(["storage", "container", "delete", "--name", container]);

        if let Some(ref account) = self.config.storage_account {
            cmd.args(["--account-name", account]);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute az storage container delete")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Container deletion failed: {}", stderr));
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
}
