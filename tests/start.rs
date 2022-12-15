use api::GetInfo;
use test_utils::{bitcoin, cockroach, knd};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_start() {
    let mut cockroach = cockroach!();
    cockroach.start().await;
    let mut bitcoin = bitcoin!();
    bitcoin.start().await;
    let mut knd = knd!(&bitcoin, &cockroach);
    knd.start().await;

    let health = knd.call_exporter("health").await.unwrap();
    assert_eq!(health, "OK");
    let pid = knd.call_exporter("pid").await.unwrap();
    assert_eq!(pid, knd.pid().unwrap().to_string());
    assert!(knd.call_exporter("metrics").await.is_ok());

    assert_eq!("OK", knd.call_rest_api("").await.unwrap());

    let result = knd.call_rest_api("v1/getinfo").await.unwrap();
    let info: GetInfo = serde_json::from_str(&result).unwrap();
    assert_eq!(0, info.block_height);
}
