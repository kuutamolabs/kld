use std::sync::Arc;

use log::info;
use s3::{creds::Credentials, Bucket, BucketConfiguration, Region};
use settings::Settings;

pub struct S3 {
    keys_bucket: Arc<Bucket>,
    graph_bucket: Arc<Bucket>,
}

#[derive(Copy, Clone)]
pub enum S3Bucket {
    Keys,
    Graph,
}

impl S3 {
    pub async fn new(settings: &Settings) -> S3 {
        info!("Connecting to object store");
        let region = Region::Custom {
            region: settings.s3_region.clone(),
            endpoint: format!("https://{}", settings.s3_address.clone()),
        };
        let credentials = Credentials::new(
            Some(&settings.s3_access_key),
            Some(&settings.s3_secret_key),
            None,
            None,
            None,
        )
        .unwrap();
        let keys_bucket = Bucket::create_with_path_style(
            &format!("{}.keys", &settings.env),
            region.clone(),
            credentials.clone(),
            BucketConfiguration::private(),
        )
        .await
        .unwrap();
        info!("Keys bucket response: {}", keys_bucket.response_text);
        let graph_bucket = Bucket::create_with_path_style(
            &format!("{}.graph", &settings.env),
            region.clone(),
            credentials.clone(),
            BucketConfiguration::private(),
        )
        .await
        .unwrap()
        .bucket;
        info!("Graph bucket response: {}", keys_bucket.response_text);
        S3 {
            keys_bucket: Arc::new(keys_bucket.bucket),
            graph_bucket: Arc::new(graph_bucket),
        }
    }

    pub async fn put(&self, bucket: S3Bucket, path: &str, content: &[u8]) {
        let bucket = self.actual(bucket);
        bucket.put_object(path, content).await.unwrap();
    }

    pub fn put_blocking(&self, bucket: S3Bucket, path: &str, content: &[u8]) {
        let bucket = self.actual(bucket);
        bucket.put_object_blocking(path, content).unwrap();
    }

    pub async fn get(&self, bucket: S3Bucket, path: &str) -> Vec<u8> {
        let bucket = self.actual(bucket);
        bucket.get_object(path).await.unwrap().into()
    }

    pub async fn list(
        &self,
        bucket: S3Bucket,
        path: &str,
        delimiter: Option<String>,
    ) -> Vec<String> {
        let bucket = self.actual(bucket);
        bucket
            .list(path.to_string(), delimiter)
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .contents
            .iter()
            .map(|x| x.key.clone())
            .collect::<Vec<String>>()
    }

    pub async fn exists(&self, bucket: S3Bucket, path: &str) -> bool {
        self.list(bucket, path, None).await.len() == 1
    }

    pub async fn delete(&self, bucket: S3Bucket, path: &str) {
        let bucket = self.actual(bucket);
        bucket.delete_object(path).await.unwrap();
    }

    fn actual(&self, bucket: S3Bucket) -> Arc<Bucket> {
        match bucket {
            S3Bucket::Keys => self.keys_bucket.clone(),
            S3Bucket::Graph => self.graph_bucket.clone(),
        }
    }
}
