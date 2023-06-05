CREATE TYPE payment_status AS ENUM ('pending', 'succeeded', 'recipient_rejected', 'user_abandoned', 'retries_exhausted', 'expired', 'route_not_found', 'error');

CREATE TYPE payment_direction AS ENUM ('inbound', 'outbound');

CREATE TABLE payments (
    id              BYTES NOT NULL,
    hash            BYTES NOT NULL,
    preimage        BYTES,
    secret          BYTES,
    status          payment_status NOT NULL,
    amount          INT NOT NULL,
    fee             INT,
    metadata        BYTES,
    direction       payment_direction NOT NULL,
    channel_id      BYTES,
    counterparty_id BYTES,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( id ),
    INDEX (hash)
);

