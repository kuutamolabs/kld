CREATE TYPE forward_status AS ENUM ('succeeded', 'failed');

CREATE TABLE forwards (
    id                   UUID NOT NULL,
    inbound_channel_id   BYTES NOT NULL,
    outbound_channel_id  BYTES,
    amount               INT,
    fee                  INT,
    status               forward_status NOT NULL,
    htlc_destination     BYTES,
    timestamp            TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( id )
);
