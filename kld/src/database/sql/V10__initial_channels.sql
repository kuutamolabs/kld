CREATE TABLE initial_channels (
    initial_channel_id                BYTES NOT NULL,
    counterparty                      BYTES NOT NULL,
    is_public                         BOOLEAN NOT NULL,
    channel_id                        BYTES,
    status                            BYTES,
    txid                              BYTES NOT NULL,
    vout                              INT4,
    open_timestamp                    TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    update_timestamp                  TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( initial_channel_id )
);
