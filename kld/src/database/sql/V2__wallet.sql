CREATE TABLE wallet_version (
	version INT
);

INSERT INTO wallet_version VALUES (1);

CREATE TABLE wallet_script_pubkeys (
	keychain TEXT,
	child INT4,
	script BLOB,
	INDEX (keychain, child),
	INDEX (script)
);

CREATE TABLE wallet_utxos (
	value INT,
	keychain TEXT,
	vout INT4,
	txid BLOB,
	script BLOB,
	is_spent BOOL,
	PRIMARY KEY (txid, vout)
);

CREATE TABLE wallet_transactions (
	txid BLOB,
	raw_tx BLOB,
	INDEX (txid)
);

CREATE TABLE wallet_transaction_details (
	txid BLOB,
	timestamp INT,
	received INT,
	sent INT,
	fee INT,
	height INT,
	INDEX (txid)
);

CREATE TABLE wallet_last_derivation_indices (
	keychain TEXT PRIMARY KEY,
	value INT
);

CREATE TABLE wallet_checksums (
	keychain TEXT,
	checksum BLOB,
	INDEX (keychain)
);

CREATE TABLE wallet_sync_time (
	id INT PRIMARY KEY,
	height INT,
	timestamp INT
);

