use test_utils::{bitcoin, knd};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_prometheus() {
    let mut bitcoin = bitcoin!();
    bitcoin.start().await;
    let mut knd = knd!(&bitcoin);
    knd.start().await;

    let health = knd.call_exporter("health").await.unwrap();
    assert_eq!(health, "OK");

    let pid = knd.call_exporter("pid").await.unwrap();
    assert_eq!(pid, knd.pid().unwrap().to_string());

    let metrics = knd.call_exporter("metrics").await.unwrap();
    assert!(get_metric(&metrics, "lightning_knd_uptime") > 0.0);
    assert_eq!(get_metric(&metrics, "lightning_node_count"), 0.0);
    assert_eq!(get_metric(&metrics, "lightning_channel_count"), 0.0);
    assert_eq!(get_metric(&metrics, "lightning_peer_count"), 0.0);

    let not_found = knd.call_exporter("wrong").await.unwrap();
    assert_eq!(not_found, "Not Found");
}

fn get_metric(metrics: &str, name: &str) -> f64 {
    metrics
        .lines()
        .find(|x| x.starts_with(name))
        .unwrap()
        .split(' ')
        .last()
        .unwrap()
        .parse::<f64>()
        .unwrap()
}
