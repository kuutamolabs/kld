use crate::components::command::parsers::parse_channel_details;

#[test]
fn test_parse_channel_details() {
    let response = r#"[
  {
    "balance_msat": 1000000000,
    "channel_id": "11a8ad563470580fedcb6c59fbcfcc9a0593d8da3b01e08a87ce3e84a80e8737",
    "channel_shutdown_state": "NotShuttingDown",
    "channel_value_satoshis": 1000000,
    "closure_reason": "Channel closed because the ChannelManager read from disk was stale compared to ChannelMonitor(s)",
    "config_accept_underpaying_htlcs": false,
    "config_cltv_expiry_delta": 72,
    "config_force_close_avoidance_max_fee_satoshis": 1000,
    "config_forwarding_fee_base_msat": 1000,
    "config_forwarding_fee_proportional_millionths": 0,
    "config_max_dust_htlc_exposure_is_fixed": false,
    "config_max_dust_htlc_exposure_value": 5000,
    "confirmations": 4,
    "confirmations_required": 3,
    "counterparty_node_id": "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
    "counterparty_outbound_htlc_maximum_msat": 450000000,
    "counterparty_outbound_htlc_minimum_msat": 1,
    "counterparty_unspendable_punishment_reserve": 10000,
    "features": [
      "required StaticRemoteKey"
    ],
    "feerate_sat_per_1000_weight": 40808,
    "force_close_spend_delay": 720,
    "funding_txo": "37870ea8843ece878ae0013bdad893059acccffb596ccbed0f58703456ada811:0",
    "has_monitor": false,
    "inbound_capacity_msat": 0,
    "inbound_htlc_maximum_msat": 980000000,
    "inbound_htlc_minimum_msat": 1,
    "inbound_scid_alias": 25531354448569286,
    "is_channel_ready": true,
    "is_outbound": true,
    "is_public": true,
    "is_usable": false,
    "next_outbound_htlc_limit_msat": 450000000,
    "next_outbound_htlc_minimum_msat": 1,
    "open_timestamp": 1702623814,
    "outbound_capacity_msat": 990000000,
    "outbound_scid_alias": 872848405330001923,
    "short_channel_id": 902984919605837824,
    "unspendable_punishment_reserve": 10000,
    "update_timestamp": 1702892393,
    "user_channel_id": 2631310026696697131
  }
]"#;
    let rows = parse_channel_details(response).expect("parse channel details should work");
    assert_eq!(rows.len(), 1);
}
