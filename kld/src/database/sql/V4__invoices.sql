CREATE TABLE invoices (
    payment_hash    BYTES NOT NULL,
    label           VARCHAR,
    expiry          INT,
    payee_pub_key   BYTES,
    amount          INT,
    bolt11          VARCHAR NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( payment_hash ),
    INDEX ( label )
);

ALTER TABLE payments ADD COLUMN label VARCHAR;
