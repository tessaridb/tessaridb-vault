//! TessariDB S3 for ciphertext attachments. The official client stays behind a small object-store boundary so
//! storage code can test finalization and failure behavior without a network.

use async_trait::async_trait;
use tessaridb_s3_client::{BucketName, Client, ClientBuilder, ObjectKey, sign::Credentials};

use crate::config::S3Config;
use crate::{Error, Result};

/// The object operations the attachment repository needs, with no credential or transport types exposed.
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    /// Store `bytes` under `key`, replacing no visible metadata by itself.
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<()>;
    /// Read and checksum-verify the whole object under `key`.
    async fn get(&self, key: &str) -> Result<Vec<u8>>;
    /// Delete the object under `key`.
    async fn delete(&self, key: &str) -> Result<()>;
}

/// One bucket and its dedicated application credentials, shared cheaply between requests.
#[derive(Clone)]
pub struct S3 {
    client: Client,
    bucket: BucketName,
}

impl S3 {
    /// Build the official client for `config`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidConfig`] when the bucket or client setup is invalid.
    pub fn new(config: &S3Config) -> Result<Self> {
        let bucket = BucketName::new(&config.bucket).map_err(|_| Error::InvalidConfig {
            field: "TESSARIDB_VAULT_S3_BUCKET",
            reason: "a valid bucket name",
        })?;
        let client = ClientBuilder::new(
            &config.endpoint,
            Credentials::new(&config.access_key, &config.secret_key),
        )
        .region(&config.region)
        .build()
        .map_err(|_| Error::InvalidConfig {
            field: "TESSARIDB_VAULT_S3_ENDPOINT",
            reason: "a valid S3 endpoint and region",
        })?;
        Ok(Self { client, bucket })
    }
}

fn key(value: &str) -> Result<ObjectKey> {
    ObjectKey::new(value).map_err(|_| Error::InvalidConfig {
        field: "S3 object key",
        reason: "a non-empty key without `.` or `..` segments",
    })
}

fn unavailable() -> Error {
    Error::Unavailable {
        message: "the S3 object store is unavailable".to_owned(),
    }
}

#[async_trait]
impl ObjectStorage for S3 {
    async fn put(&self, key_value: &str, bytes: &[u8]) -> Result<()> {
        self.client
            .put_object(&self.bucket, &key(key_value)?, bytes.to_vec(), None)
            .await
            .map_err(|_| unavailable())?;
        Ok(())
    }

    async fn get(&self, key_value: &str) -> Result<Vec<u8>> {
        self.client
            .get_object(&self.bucket, &key(key_value)?)
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|_| unavailable())
    }

    async fn delete(&self, key_value: &str) -> Result<()> {
        self.client
            .delete_object(&self.bucket, &key(key_value)?)
            .await
            .map_err(|_| unavailable())
    }
}
