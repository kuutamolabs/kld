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
use macaroon::{Macaroon, MacaroonKey, Verifier};

pub struct MacaroonAuth {
    key: MacaroonKey,
}

impl MacaroonAuth {
    pub fn init(seed: &[u8; 32], data_dir: &str) -> Result<MacaroonAuth> {
        macaroon::initialize()?;
        let key = MacaroonKey::generate(seed);

        let admin_macaroon = Self::admin_macaroon(&key)?;
        let readonly_macaroon = Self::readonly_macaroon(&key)?;
        fs::create_dir_all(format!("{}/macaroons", data_dir))?;
        fs::write(
            format!("{}/macaroons/admin_macaroon", data_dir),
            admin_macaroon.serialize(macaroon::Format::V2)?,
        )?;
        fs::write(
            format!("{}/macaroons/readonly_macaroon", data_dir),
            readonly_macaroon.serialize(macaroon::Format::V2)?,
        )?;

        Ok(MacaroonAuth { key })
    }

    pub fn verify_macaroon(&self, macaroon: &Macaroon) -> Result<()> {
        let mut verifier = Verifier::default();
        verifier.satisfy_exact("isadmin".into());
        Ok(verifier.verify(macaroon, &self.key, vec![])?)
    }

    fn admin_macaroon(key: &MacaroonKey) -> Result<Macaroon> {
        Ok(Macaroon::create(None, key, "admin".into())?)
    }

    fn readonly_macaroon(key: &MacaroonKey) -> Result<Macaroon> {
        Ok(Macaroon::create(None, key, "readonly".into())?)
    }
}

pub struct KndMacaroon(pub Macaroon);

#[async_trait]
impl<S> FromRequestParts<S> for KndMacaroon
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(value) = parts.headers.get("macaroon") {
            Macaroon::deserialize(value)
                .map(KndMacaroon)
                .map_err(|_| (StatusCode::UNAUTHORIZED, "Unable to deserialize macaroon"))
        } else {
            Err((StatusCode::UNAUTHORIZED, "Missing macaroon header"))
        }
    }
}

#[test]
fn macaroon_test() {
    let macaroon_auth = MacaroonAuth::init(&[3u8; 32], "").unwrap();
    let admin_macaroon = MacaroonAuth::admin_macaroon(&macaroon_auth.key).unwrap();
    let readonly_macaroon = MacaroonAuth::readonly_macaroon(&macaroon_auth.key).unwrap();

    assert!(macaroon_auth.verify_macaroon(&admin_macaroon).is_ok());
    assert!(macaroon_auth.verify_macaroon(&readonly_macaroon).is_ok());
}
