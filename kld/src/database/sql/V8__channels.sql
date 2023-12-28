CREATE TABLE channels (
    channel_id                        BYTES NOT NULL,
    counterparty                      BYTES NOT NULL,

    /* The default channel config may not be consist in the future
       So we need to keep recording the data version */
    data_version                      INT DEFAULT 0,
    /* 0 - lightning 0.0.118 */

    short_channel_id                  INT,
    is_usable                         BOOLEAN NOT NULL,
    is_public                         BOOLEAN NOT NULL,

    /* The data of ChannelDetails */
    data                              BYTES,

    /* Kuutamo customized fields */
    open_timestamp                    TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    update_timestamp                  TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    closure_reason                    BYTES,

    PRIMARY KEY ( channel_id )
);
