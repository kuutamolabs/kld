CREATE TYPE spendable_output_status AS ENUM ('unspent', 'spent');

CREATE TABLE spendable_outputs (
    txid            BYTES NOT NULL,
    "index"         INT2 NOT NULL,
    value           INT NOT NULL,
    descriptor      BYTES NOT NULL,
    status          spendable_output_status NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( txid, "index" )
);
