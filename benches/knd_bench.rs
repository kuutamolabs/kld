extern crate criterion;
use anyhow::Result;
use criterion::{criterion_group, criterion_main, Criterion};
use database::ldk_database::LdkDatabase;
use database::migrate_database;
use lightning::ln::features::InitFeatures;
use lightning::ln::functional_test_utils::{
    create_announced_chan_between_nodes, create_chanmon_cfgs, create_network, create_node_cfgs,
    create_node_chanmgrs, send_payment,
};
use lightning::util::logger::Level::Warn;
use lightning::util::test_utils::TestChainMonitor;
use test_utils::{cockroach, TestSettingsBuilder};

criterion_group! {
    name = benches;
    config = Criterion::default().significance_level(0.1).sample_size(10).measurement_time(std::time::Duration::from_secs(30));
    targets = bench_send_payment_two_nodes
}
criterion_main!(benches);

// we add wrapper functions like that to only unwrap in one place and still cleanup all ressources.
pub fn bench_send_payment_two_nodes(c: &mut Criterion) {
    send_payment_two_nodes(c).unwrap()
}

/// Send one payment between two nodes with two cockroach instances.
/// The functional_test_utils just calls the message handlers on each node, no network involved.
pub fn send_payment_two_nodes(c: &mut Criterion) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()?;

    let mut cockroach_0 = cockroach!();
    runtime.block_on(cockroach_0.start())?;
    let settings_0 = TestSettingsBuilder::new()
        .with_database_port(cockroach_0.sql_port)
        .build();
    runtime.block_on(migrate_database(&settings_0))?;
    let db_0 = runtime.block_on(LdkDatabase::new(&settings_0))?;

    let mut cockroach_1 = cockroach!(1);
    runtime.block_on(cockroach_1.start())?;
    let settings_1 = TestSettingsBuilder::new()
        .with_database_port(cockroach_1.sql_port)
        .build();
    runtime.block_on(migrate_database(&settings_1))?;
    let db_1 = runtime.block_on(LdkDatabase::new(&settings_1))?;

    let mut chanmon_cfgs = create_chanmon_cfgs(2);
    chanmon_cfgs[0].logger.enable(Warn);
    chanmon_cfgs[1].logger.enable(Warn);
    let mut node_cfgs = create_node_cfgs(2, &chanmon_cfgs);

    let chain_mon_0 = TestChainMonitor::new(
        Some(&chanmon_cfgs[0].chain_source),
        &chanmon_cfgs[0].tx_broadcaster,
        &chanmon_cfgs[0].logger,
        &chanmon_cfgs[0].fee_estimator,
        &db_0,
        node_cfgs[0].keys_manager,
    );
    let chain_mon_1 = TestChainMonitor::new(
        Some(&chanmon_cfgs[1].chain_source),
        &chanmon_cfgs[1].tx_broadcaster,
        &chanmon_cfgs[1].logger,
        &chanmon_cfgs[1].fee_estimator,
        &db_1,
        node_cfgs[1].keys_manager,
    );
    node_cfgs[0].chain_monitor = chain_mon_0;
    node_cfgs[1].chain_monitor = chain_mon_1;
    let node_chanmgrs = create_node_chanmgrs(2, &node_cfgs, &[None, None]);
    let nodes = create_network(2, &node_cfgs, &node_chanmgrs);

    let _ = create_announced_chan_between_nodes(
        &nodes,
        0,
        1,
        InitFeatures::empty(),
        InitFeatures::empty(),
    );

    c.bench_function("send_payment_two_nodes", |b| {
        b.iter(|| {
            send_payment(&nodes[0], &vec![&nodes[1]][..], 1000);
        });
    });
    Ok(())
}
