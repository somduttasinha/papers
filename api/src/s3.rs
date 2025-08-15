// utility wrapper functions to interact with the S3 API

use std::{error::Error, time::Duration};

use aws_sdk_s3::{Client, presigning::PresigningConfig, primitives::ByteStream};

pub async fn ensure_bucket(
    client: &Client,
    bucket: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use aws_sdk_s3::types::BucketLocationConstraint;
    if client.head_bucket().bucket(bucket).send().await.is_ok() {
        return Ok(());
    }
    // Region is mostly ignored by MinIO; pick your SDK region
    client
        .create_bucket()
        .bucket(bucket)
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
    client: &aws_sdk_s3::Client,
    bucket_name: &str,
    content_type: &str,
    key: &str,
    bytes: ByteStream,
) -> Result<aws_sdk_s3::operation::put_object::PutObjectOutput, Box<dyn std::error::Error>> {
    client
        .put_object()
        .bucket(bucket_name)
        .key(key)
        .body(bytes)
        .content_type(content_type)
        .send()
        .await
        .map_err(Into::into)
}

/// Generate a URL for a presigned GET request.
pub async fn get_object(
    client: &Client,
    bucket: &str,
    key: &str,
    expires_in: u64,
) -> Result<String, Box<dyn Error>> {
    let expires_in = Duration::from_secs(expires_in);
    let presigned_request = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(PresigningConfig::expires_in(expires_in)?)
        .await?;

    println!("Object URI: {}", presigned_request.uri());
    let valid_until = chrono::offset::Local::now() + expires_in;
    println!("Valid until: {valid_until}");

    Ok(presigned_request.uri().to_string())
}
