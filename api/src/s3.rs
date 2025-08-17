// utility wrapper functions to interact with the S3 API

use std::{error::Error, time::Duration};

use aws_sdk_s3::{
    Client, operation::get_object::GetObjectOutput, presigning::PresigningConfig,
    primitives::ByteStream,
};

pub struct S3Client {
    client: Client,
    bucket: String,
}

impl S3Client {
    pub fn new(client: Client, bucket: String) -> Self {
        S3Client { client, bucket }
    }

    pub async fn ensure_bucket(&self) -> Result<(), Box<dyn std::error::Error>> {
        use aws_sdk_s3::types::BucketLocationConstraint;
        if self
            .client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .is_ok()
        {
            return Ok(());
        }
        // Region is mostly ignored by MinIO; pick your SDK region
        self.client
            .create_bucket()
            .bucket(&self.bucket)
            .create_bucket_configuration(
                aws_sdk_s3::types::CreateBucketConfiguration::builder()
                    .location_constraint(BucketLocationConstraint::UsGovEast1)
                    .build(),
            )
            .send()
            .await?;
        Ok(())
    }

    pub async fn upload_object(
        &self,
        content_type: &str,
        key: &str,
        bytes: ByteStream,
    ) -> Result<aws_sdk_s3::operation::put_object::PutObjectOutput, Box<dyn std::error::Error>>
    {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(bytes)
            .content_type(content_type)
            .send()
            .await
            .map_err(Into::into)
    }

    /// Generate a URL for a presigned GET request.
    pub async fn get_object_url(
        &self,
        key: &str,
        expires_in: u64,
    ) -> Result<String, Box<dyn Error>> {
        let expires_in = Duration::from_secs(expires_in);
        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(PresigningConfig::expires_in(expires_in)?)
            .await?;

        println!("Object URI: {}", presigned_request.uri());
        let valid_until = chrono::offset::Local::now() + expires_in;
        println!("Valid until: {valid_until}");

        Ok(presigned_request.uri().to_string())
    }

    pub async fn get_object(&self, key: &str) -> Result<GetObjectOutput, Box<dyn Error>> {
        self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(Into::into)
    }
}

// function with lifetime

// function with lifetime
