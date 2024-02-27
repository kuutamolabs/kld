use color_eyre::eyre::Result;
use kld::api::codegen::get_kld_channel_response::GetKldChannelResponseItem;
use ratatui::{prelude::*, widgets::*};

use crate::utils::{ts_to_string, WORD_BINDINGS};

pub fn parse_channel_details<'a>(
    input: impl std::convert::AsRef<str>,
) -> Result<Vec<Vec<Row<'a>>>> {
    let details: Vec<GetKldChannelResponseItem> = serde_json::from_str(input.as_ref())?;

    // XXX i18n on these fields
    let mut outputs = Vec::new();
    for detail in details.into_iter() {
        let mut output = vec![Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Channel ID"))).style(Style::default().bold()),
            Cell::from(Text::from(detail.channel_id)),
        ])];
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Short Channel ID")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .short_channel_id
                    .map(|id| id.to_string())
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("User Channel ID")))
                .style(Style::default().bold()),
            Cell::from(Text::from(detail.user_channel_id.to_string())),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Funding TXO"))).style(Style::default().bold()),
            Cell::from(Text::from(detail.funding_txo)),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Open At"))).style(Style::default().bold()),
            Cell::from(Text::from(ts_to_string(detail.open_timestamp as u64))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Updated At"))).style(Style::default().bold()),
            Cell::from(Text::from(ts_to_string(detail.update_timestamp as u64))),
        ]));
        let mut states = Vec::new();
        if let Some(state) = detail.channel_shutdown_state {
            states.push(state);
        }
        if detail.has_monitor {
            states.push("has monitor".into());
        } else {
            states.push("no monitor".into());
        }
        if detail.is_channel_ready {
            states.push("is ready".into());
        } else {
            states.push("not ready".into());
        }
        if detail.is_outbound {
            states.push("outbound".into());
        } else {
            states.push("inbound".into());
        }
        if detail.is_public {
            states.push("public".into());
        } else {
            states.push("private".into());
        }
        if detail.is_usable {
            states.push("usable".into());
        } else {
            states.push("not usable".into());
        }
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("State"))).style(Style::default().bold()),
            Cell::from(Text::from(states.join("/"))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Closure Reason")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail.closure_reason.unwrap_or("none".to_string()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Balance"))).style(Style::default().bold()),
            Cell::from(Text::from(format!("{} msats", detail.balance_msat))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Value"))).style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} sats",
                detail.channel_value_satoshis
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Confirmations")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .confirmations
                    .map(|c| format!("{} blocks", c))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Required Confirmations")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .confirmations_required
                    .map(|c| format!("{} blocks", c))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Features"))).style(Style::default().bold()),
            Cell::from(Text::from(detail.features.join(","))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Feerate"))).style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .feerate_sat_per_1000_weight
                    .map(|c| format!("{c} sats / 1000 weight"))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Force Close Spend Delay")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .force_close_spend_delay
                    .map(|c| format!("{c} blocks"))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(
                WORD_BINDINGS.get("Unspendable Punishment Reserve"),
            ))
            .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .unspendable_punishment_reserve
                    .map(|c| format!("{c} sats",))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![""]));
        // Inbound
        output.push(Row::new(vec![Cell::from(Text::from(
            WORD_BINDINGS.get("Inbound"),
        ))
        .style(Style::default().bold())]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Capacity"))).style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} msats",
                detail.inbound_capacity_msat
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("HTLC Maximum")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .inbound_htlc_maximum_msat
                    .map(|s| format!("{s} msats"))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("HTLC Minimum")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .inbound_htlc_minimum_msat
                    .map(|s| format!("{s} msats"))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Short Channel ID Alias")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .inbound_scid_alias
                    .map(|id| id.to_string())
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![""]));
        // Outbound
        output.push(Row::new(vec![Cell::from(Text::from(
            WORD_BINDINGS.get("Inbound"),
        ))
        .style(Style::default().bold())]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Capacity"))).style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} msats",
                detail.outbound_capacity_msat
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Short Channel ID Alias")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .outbound_scid_alias
                    .map(|id| id.to_string())
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Next HTLC Maximum")))
                .style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} msats",
                detail.next_outbound_htlc_limit_msat
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Next HTLC Minimum")))
                .style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} msats",
                detail.next_outbound_htlc_minimum_msat
            ))),
        ]));
        output.push(Row::new(vec![""]));
        // Config
        output.push(Row::new(vec![Cell::from(Text::from(
            WORD_BINDINGS.get("Config"),
        ))
        .style(Style::default().bold())]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Underpaying HTLC")))
                .style(Style::default().bold()),
            Cell::from(Text::from(if detail.config_accept_underpaying_htlcs {
                WORD_BINDINGS.get("Accept")
            } else {
                WORD_BINDINGS.get("Deny")
            })),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("CLTV Expiry"))).style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} blocks",
                detail.config_cltv_expiry_delta
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(
                WORD_BINDINGS.get("Force Close Avoidance Max Fee"),
            ))
            .style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} sats",
                detail.config_force_close_avoidance_max_fee_satoshis
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Forwarding Fee Base")))
                .style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} msats",
                detail.config_forwarding_fee_base_msat
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Forwarding Fee")))
                .style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} sat/proportional millionths",
                detail.config_forwarding_fee_proportional_millionths
            ))),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Max Dust HTLC Exposure")))
                .style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} {}",
                if detail.config_max_dust_htlc_exposure_is_fixed {
                    WORD_BINDINGS.get("fix with")
                } else {
                    WORD_BINDINGS.get("fee rate multiplied by")
                },
                detail.config_max_dust_htlc_exposure_value
            ))),
        ]));
        output.push(Row::new(vec![""]));
        // Counterparty
        output.push(Row::new(vec![Cell::from(Text::from(
            WORD_BINDINGS.get("Counterparty"),
        ))
        .style(Style::default().bold())]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Node Id"))).style(Style::default().bold()),
            Cell::from(Text::from(detail.counterparty_node_id)),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Outbound HTLC Maximum")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .counterparty_outbound_htlc_maximum_msat
                    .map(|c| format!("{c} msats"))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(WORD_BINDINGS.get("Outbound HTLC Minimum")))
                .style(Style::default().bold()),
            Cell::from(Text::from(
                detail
                    .counterparty_outbound_htlc_minimum_msat
                    .map(|c| format!("{c} msats"))
                    .unwrap_or("none".into()),
            )),
        ]));
        output.push(Row::new(vec![
            Cell::from(Text::from(
                WORD_BINDINGS.get("Unspendable Punishment Reserve"),
            ))
            .style(Style::default().bold()),
            Cell::from(Text::from(format!(
                "{} sats",
                detail.counterparty_unspendable_punishment_reserve
            ))),
        ]));
        outputs.push(output);
    }
    Ok(outputs)
}
