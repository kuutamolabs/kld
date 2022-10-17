pub mod bitcoin_manager;
pub mod knd_manager;

#[macro_export]
macro_rules! poll {
    ($secs: expr, $func: expr) => {
        let mut ellapsed = 0;
        while ellapsed < $secs {
            if $func {
                break;
            };
            tokio::time::sleep(Duration::from_secs(1)).await;
            ellapsed += 1;
        }
        if ellapsed == $secs {
            panic!("Timed out polling for result");
        }
    };
}
