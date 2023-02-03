CREATE TABLE channel_manager (
    id              BYTES PRIMARY KEY,
    manager         BYTES NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp()
);

CREATE TABLE channel_monitors (
    out_point       BYTES NOT NULL,
    update_id       INT NOT NULL,
    monitor         BYTES NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( out_point )
);

CREATE TABLE channel_monitor_updates (
    out_point       BYTES NOT NULL,
    update          BYTES NOT NULL,
    update_id       INT NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    PRIMARY KEY ( out_point, update_id )
);

CREATE TABLE network_graph (
    id              BYTES PRIMARY KEY,
    graph           BYTES NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp()
);

CREATE TABLE scorer (
    id              BYTES PRIMARY KEY,
    scorer          BYTES NOT NULL,
    timestamp       TIMESTAMP NOT NULL DEFAULT current_timestamp()
);

CREATE TABLE peers (
    public_key      BYTES NOT NULL,
    address         BYTES NOT NULL,
    PRIMARY KEY ( public_key, address )
);
