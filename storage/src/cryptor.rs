use aes_gcm_siv::aead::{Aead, AeadCore, OsRng};
use aes_gcm_siv::{Aes256GcmSiv, Key, KeyInit, Nonce};
use settings::Settings;

pub struct Cryptor {
    cipher: Aes256GcmSiv,
}

impl Cryptor {
    pub fn new(settings: &Settings) -> Cryptor {
        let key = Key::<Aes256GcmSiv>::from_slice(&settings.s3_encryption_key.as_ref());
        let cipher = Aes256GcmSiv::new(&key);

        Cryptor { cipher }
    }

    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let nonce = Aes256GcmSiv::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, data)
            .expect("encryption failure!");
        let mut result = nonce.to_vec();
        result.extend(&ciphertext);
        result
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Vec<u8> {
        let (nonce, ciphertext) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .decrypt(nonce, ciphertext.as_ref())
            .expect("decryption failure!")
    }
}

#[cfg(test)]
mod test {
    use test_utils::test_settings;

    use crate::cryptor::Cryptor;

    #[test]
    pub fn test_cryptor() {
        let settings = test_settings();

        let crypt = Cryptor::new(&settings);
        let message = b"plaintext message plaintext message plaintext message";
        let ciphertext = crypt.encrypt(message);

        let crypt = Cryptor::new(&settings);
        let plaintext = crypt.decrypt(&ciphertext);
        assert_eq!(&plaintext, message);
    }
}
