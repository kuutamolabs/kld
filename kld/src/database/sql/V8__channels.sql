CREATE TABLE channels (
    id                   BYTES NOT NULL,
    scid                 INT NOT NULL,
    user_channel_id      INT NOT NULL,
    counterparty         BYTES NOT NULL,
    funding_txo          BYTES NOT NULL,
    is_public            BOOLEAN NOT NULL,
    is_outbound          BOOLEAN NOT NULL,
    value                INT NOT NULL,
    type_features        BYTES NOT NULL,
    open_timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    close_timestamp      TIMESTAMP,
    closure_reason       BYTES,
    PRIMARY KEY ( id )
);
