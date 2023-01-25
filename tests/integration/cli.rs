use std::process::{Command, Output};

use anyhow::{bail, Result};
use api::{
    Channel, FundChannelResponse, GetInfo, NewAddressResponse, Peer, WalletBalance,
    WalletTransferResponse,
};
use bitcoin::secp256k1::PublicKey;
use serde::de;

use crate::mocks::{TEST_ADDRESS, TEST_PUBLIC_KEY};

use super::api::API_SETTINGS;

#[test]
fn test_cli_get_info() -> Result<()> {
    let output = run_cli("get-info", &[]);
    let _: GetInfo = deserialize(&output.stdout)?;
    Ok(())
}

#[test]
fn test_cli_get_balance() -> Result<()> {
    let output = run_cli("get-balance", &[]);
    let _: WalletBalance = deserialize(&output.stdout)?;
    Ok(())
}

#[test]
fn test_cli_new_address() -> Result<()> {
    let output = run_cli("new-address", &[]);
    let _: NewAddressResponse = deserialize(&output.stdout)?;
    Ok(())
}

#[test]
fn test_cli_withdraw() -> Result<()> {
    let output = run_cli(
        "withdraw",
        &["--address", TEST_ADDRESS, "--satoshis", "1000"],
    );
    let _: WalletTransferResponse = deserialize(&output.stdout)?;
    Ok(())
}

#[test]
fn test_cli_list_channels() -> Result<()> {
    let output = run_cli("list-channels", &[]);
    let _: Vec<Channel> = deserialize(&output.stdout)?;
    Ok(())
}

#[test]
fn test_cli_list_peers() -> Result<()> {
    let output = run_cli("list-peers", &[]);
    let _: Vec<Peer> = deserialize(&output.stdout)?;
    Ok(())
}

#[test]
fn test_cli_connect_peer() -> Result<()> {
    let output = run_cli("connect-peer", &["--public-key", TEST_PUBLIC_KEY]);
    let _: PublicKey = deserialize(&output.stdout)?;
    Ok(())
}

#[test]
fn test_cli_disconnect_peer() -> Result<()> {
    let output = run_cli("disconnect-peer", &["--public-key", TEST_PUBLIC_KEY]);

    deserialize(&output.stdout)
}

#[test]
fn test_cli_open_channel() -> Result<()> {
    let output = run_cli(
        "open-channel",
        &["--public-key", TEST_PUBLIC_KEY, "--satoshis", "1000"],
    );
    let _: FundChannelResponse = deserialize(&output.stdout)?;
    Ok(())
}

fn deserialize<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    match serde_json::from_slice::<T>(bytes) {
        Ok(t) => Ok(t),
        Err(e) => {
            let s = String::from_utf8_lossy(bytes);
            bail!("cannot parse '{}' as json: {}", s, e)
        }
    }
}

fn run_cli(command: &str, extra_args: &[&str]) -> Output {
    let settings = &API_SETTINGS;

    let output = Command::new(env!("CARGO_BIN_EXE_lightning-knd-cli"))
        .args([
            "--target",
            &settings.rest_api_address,
            "--cert-path",
            &format!("{}/knd.crt", settings.certs_dir),
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
    output
}
