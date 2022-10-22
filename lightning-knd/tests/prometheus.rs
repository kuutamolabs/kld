use test_utils::{bitcoin, knd};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_prometheus() {
    let mut bitcoin = bitcoin!();
    bitcoin.start();
    let mut knd = knd!(&bitcoin);
    knd.start().await;

    let health = knd.call_exporter("health").await.unwrap();
    assert_eq!(health, "OK");

    let pid = knd.call_exporter("pid").await.unwrap();
    assert_eq!(pid, knd.pid().unwrap().to_string());

    let metrics = knd.call_exporter("metrics").await.unwrap();
    assert!(metrics
        .lines()
        .last()
        .unwrap()
        .starts_with("lightning_knd_uptime"));

    let not_found = knd.call_exporter("wrong").await.unwrap();
    assert_eq!(not_found, "Not Found");
}
