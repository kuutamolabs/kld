CREATE TABLE spendable_outputs (
    txid            BYTES NOT NULL,
    "index"         INT2 NOT NULL,
    value           INT NOT NULL,
    channel_id      BYTES,

    /* The data of SpendableOutputDescriptor */
    data            BYTES NOT NULL,

    is_spent        BOOL NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( txid, "index" )
);
