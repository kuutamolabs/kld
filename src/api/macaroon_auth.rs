use base64::{engine::general_purpose, Engine};
use hyper::header;
#[cfg(not(test))]
use std::fs;
#[cfg(test)]
use test_utils::fake_fs as fs;

use anyhow::Result;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use macaroon::{ByteString, Macaroon, MacaroonKey, Verifier};

pub struct MacaroonAuth {
    key: MacaroonKey,
}

impl MacaroonAuth {
    pub fn init(seed: &[u8; 32], data_dir: &str) -> Result<MacaroonAuth> {
        macaroon::initialize()?;
        let key = MacaroonKey::generate(seed);

        let admin_macaroon = Self::admin_macaroon(&key)?;
        let readonly_macaroon = Self::readonly_macaroon(&key)?;

        let mut buf = vec![];
        let base64 = admin_macaroon.serialize(macaroon::Format::V2)?;
        general_purpose::URL_SAFE.decode_vec(base64, &mut buf)?;

        fs::create_dir_all(format!("{}/macaroons", data_dir))?;
        // access.macaroon is compatible with CLN
        fs::write(format!("{}/macaroons/access.macaroon", data_dir), &buf)?;
        // admin.macaroon is compatible with LND
        fs::write(
            format!("{}/macaroons/admin.macaroon", data_dir),
            admin_macaroon.serialize(macaroon::Format::V2)?,
        )?;
        fs::write(
            format!("{}/macaroons/readonly.macaroon", data_dir),
            readonly_macaroon.serialize(macaroon::Format::V2)?,
        )?;

        Ok(MacaroonAuth { key })
    }

    pub fn verify_admin_macaroon(&self, macaroon: &Macaroon) -> Result<()> {
        let mut verifier = Verifier::default();
        verifier.satisfy_general(|caveat| verify_role(caveat, "admin"));
        Ok(verifier.verify(macaroon, &self.key, vec![])?)
    }

    pub fn verify_readonly_macaroon(&self, macaroon: &Macaroon) -> Result<()> {
        let mut verifier = Verifier::default();
        verifier.satisfy_general(|caveat| verify_role(caveat, "readonly"));
        Ok(verifier.verify(macaroon, &self.key, vec![])?)
    }

    fn admin_macaroon(key: &MacaroonKey) -> Result<Macaroon> {
        let mut macaroon = Macaroon::create(None, key, "admin".into())?;
        macaroon.add_first_party_caveat("roles = admin|readonly".into());
        Ok(macaroon)
    }

    fn readonly_macaroon(key: &MacaroonKey) -> Result<Macaroon> {
        let mut macaroon = Macaroon::create(None, key, "readonly".into())?;
        macaroon.add_first_party_caveat("roles = readonly".into());
        Ok(macaroon)
    }
}

fn verify_role(caveat: &ByteString, expected_role: &str) -> bool {
    if !caveat.0.starts_with(b"roles = ") {
        return false;
    }
    let strcaveat = match std::str::from_utf8(&caveat.0) {
        Ok(s) => s,
        Err(_) => return false,
    };

    strcaveat[8..].split('|').any(|r| r == expected_role)
}

pub struct KndMacaroon(pub Macaroon);

#[async_trait]
impl<S> FromRequestParts<S> for KndMacaroon
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    // May as well try to decode both base64 and hex macaroons.
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let deserialize_err = (StatusCode::UNAUTHORIZED, "Unable to deserialize macaroon");

        let value = if let Some(value) = parts.headers.get(header::SEC_WEBSOCKET_PROTOCOL) {
            value
                .to_str()
                .map_err(|_| deserialize_err)?
                .split_once(',')
                .map(|s| s.0)
                .unwrap_or_default()
        } else {
            parts
                .headers
                .get("macaroon")
                .or_else(|| parts.headers.get("Grpc-Metadata-macaroon"))
                .ok_or((StatusCode::UNAUTHORIZED, "Missing macaroon header"))?
                .to_str()
                .map_err(|_| deserialize_err)?
        };

        let macaroon = Macaroon::deserialize(value).map(KndMacaroon);

        if macaroon.is_err() {
            if let Ok(bytes) = hex::decode(value) {
                if let Ok(macaroon) = Macaroon::deserialize_binary(&bytes).map(KndMacaroon) {
                    return Ok(macaroon);
                }
            }
        }

        macaroon.map_err(|_| deserialize_err)
    }
}

#[test]
fn test_readonly_macaroon() {
    let macaroon_auth = MacaroonAuth::init(&[3u8; 32], "").unwrap();
    let readonly_macaroon = MacaroonAuth::readonly_macaroon(&macaroon_auth.key).unwrap();

    macaroon_auth
        .verify_readonly_macaroon(&readonly_macaroon)
        .unwrap();
}

#[test]
fn test_admin_macaroon() {
    let macaroon_auth = MacaroonAuth::init(&[3u8; 32], "").unwrap();
    let admin_macaroon = MacaroonAuth::admin_macaroon(&macaroon_auth.key).unwrap();

    macaroon_auth
        .verify_admin_macaroon(&admin_macaroon)
        .unwrap();
}
