use test_utils::{bitcoin, knd};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_start() {
    let mut bitcoin = bitcoin!();
    bitcoin.start();
    let mut knd = knd!(&bitcoin);
    knd.start();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
}
