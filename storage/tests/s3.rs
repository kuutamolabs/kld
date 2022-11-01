use storage::object::ObjectStorage;
use test_utils::{minio, test_settings};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_persist_key() {
    let mut minio = minio!();
    minio.start().await;

    let settings = test_settings();
    let storage = ObjectStorage::new(&settings).await;
    let key = [1u8; 32];
    storage.persist_key(&key).await;
    let result = storage.read_key().await;
    assert_eq!(key, result);
    storage.delete_all().await;
    assert!(!storage.key_exists().await);
}
