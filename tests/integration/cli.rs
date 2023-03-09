use std::process::{Command, Output};

use anyhow::{bail, Result};
use api::{
    Channel, FundChannelResponse, GetInfo, NewAddressResponse, Node, Peer, SetChannelFeeResponse,
    WalletBalance, WalletTransferResponse,
};
use bitcoin::secp256k1::PublicKey;
use hyper::StatusCode;
use serde::de;

use crate::mocks::{TEST_ADDRESS, TEST_PUBLIC_KEY, TEST_SHORT_CHANNEL_ID};

use super::api::create_api_server;

#[tokio::test]
async fn test_cli_get_info() -> Result<()> {
    let output = run_cli("get-info", &[]).await?;
    let _: GetInfo = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_get_balance() -> Result<()> {
    let output = run_cli("get-balance", &[]).await?;
    let _: WalletBalance = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_new_address() -> Result<()> {
    let output = run_cli("new-address", &[]).await?;
    let _: NewAddressResponse = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_withdraw() -> Result<()> {
    let output = run_cli(
        "withdraw",
        &["--address", TEST_ADDRESS, "--satoshis", "1000"],
    )
    .await?;
    let _: WalletTransferResponse = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_list_channels() -> Result<()> {
    let output = run_cli("list-channels", &[]).await?;
    let _: Vec<Channel> = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_list_peers() -> Result<()> {
    let output = run_cli("list-peers", &[]).await?;
    let _: Vec<Peer> = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_connect_peer() -> Result<()> {
    let output = run_cli("connect-peer", &["--public-key", TEST_PUBLIC_KEY]).await?;
    let _: PublicKey = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_connect_peer_malformed_id() -> Result<()> {
    let output = run_cli("connect-peer", &["--public-key", "abc@1.2"]).await?;
    let s = String::from_utf8_lossy(&output.stdout);
    assert!(s.starts_with(&StatusCode::BAD_REQUEST.to_string()));
    Ok(())
}

#[tokio::test]
async fn test_cli_disconnect_peer() -> Result<()> {
    let output = run_cli("disconnect-peer", &["--public-key", TEST_PUBLIC_KEY]).await?;

    assert!(&output.stdout.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_cli_open_channel() -> Result<()> {
    let output = run_cli(
        "open-channel",
        &["--public-key", TEST_PUBLIC_KEY, "--sats", "1000"],
    )
    .await?;
    let _: FundChannelResponse = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_set_channel_fee() -> Result<()> {
    let output = run_cli(
        "set-channel-fee",
        &[
            "--id",
            &TEST_SHORT_CHANNEL_ID.to_string(),
            "--base-fee",
            "1000",
            "--ppm-fee",
            "200",
        ],
    )
    .await?;
    let _: SetChannelFeeResponse = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_set_channel_fee_all() -> Result<()> {
    let output = run_cli(
        "set-channel-fee",
        &["--id", "all", "--base-fee", "1000", "--ppm-fee", "200"],
    )
    .await?;
    let _: SetChannelFeeResponse = deserialize(&output.stdout)?;
    Ok(())
}

#[tokio::test]
async fn test_cli_close_channel() -> Result<()> {
    let output = run_cli(
        "close-channel",
        &["--id", &TEST_SHORT_CHANNEL_ID.to_string()],
    )
    .await?;
    assert!(output.stdout.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_cli_get_node() -> Result<()> {
    let output = run_cli("list-nodes", &["--id", TEST_PUBLIC_KEY]).await?;
    let nodes: Vec<Node> = deserialize(&output.stdout)?;
    assert!(!nodes.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_cli_list_nodes() -> Result<()> {
    let output = run_cli("list-nodes", &[]).await?;
    let nodes: Vec<Node> = deserialize(&output.stdout)?;
    assert!(!nodes.is_empty());
    Ok(())
}

fn deserialize<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    match serde_json::from_slice::<T>(bytes) {
        Ok(t) => Ok(t),
        Err(_) => {
            let s = String::from_utf8_lossy(bytes);
            bail!("Expected json output, but got: {}", s)
        }
    }
}

async fn run_cli(command: &str, extra_args: &[&str]) -> Result<Output> {
    let settings = create_api_server().await?;

    let output = Command::new(env!("CARGO_BIN_EXE_kld-cli"))
        .args([
            "--target",
            &settings.rest_api_address,
            "--cert-path",
            &format!("{}/kld.crt", settings.certs_dir),
            "--macaroon-path",
            &format!("{}/macaroons/admin.macaroon", settings.data_dir),
            command,
        ])
        .args(extra_args)
        .output()
        .unwrap();

    if !output.status.success() {
        panic!("{}", String::from_utf8(output.stderr).unwrap());
    }
    Ok(output)
}
