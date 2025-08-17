use crate::util;
use in_toto::{
    crypto::{PrivateKey, PublicKey},
    models::{Metablock, MetadataWrapper},
};
use rebuilderd_common::errors::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    pub metablock: Metablock,
}

impl Attestation {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let metablock = serde_json::from_slice::<Metablock>(bytes)?;
        Ok(Self { metablock })
    }

    pub fn sign(&mut self, privkey: &PrivateKey) -> Result<()> {
        let new = Metablock::new(self.metablock.metadata.clone(), &[privkey])?;
        self.metablock.signatures.extend(new.signatures);
        Ok(())
    }

    pub fn verify<'a, I>(&self, threshold: u32, authorized_keys: I) -> Result<MetadataWrapper>
    where
        I: IntoIterator<Item = &'a PublicKey>,
    {
        let metadata = self.metablock.verify(threshold, authorized_keys)?;
        Ok(metadata)
    }

    pub fn serialize(&self) -> Result<String> {
        serde_json::to_string(&self.metablock).context("Failed to serialize attestation")
    }

    pub async fn to_compressed_bytes(&self) -> Result<Vec<u8>> {
        let json = self.serialize()?;
        let compressed = util::zstd_compress(json.as_bytes()).await?;
        Ok(compressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_encoding::HEXLOWER;
    use in_toto::{
        crypto::{HashAlgorithm, HashValue, KeyType, Signature, SignatureScheme},
        models::{LinkMetadata, MetadataWrapper, VirtualTargetPath},
    };
    use serde_json::Value;

    // temporary until https://github.com/in-toto/in-toto-rs/pull/111 lands
    fn hashvalue_from_hex(hex: &str) -> Result<HashValue> {
        let bytes = HEXLOWER.decode(hex.as_bytes())?;
        Ok(HashValue::new(bytes))
    }

    // temporary until https://github.com/in-toto/in-toto-rs/pull/111 lands
    fn signature(keyid: &str, value: &str) -> Signature {
        let value = Value::Object(
            [
                ("keyid".to_string(), Value::String(keyid.to_string())),
                ("sig".to_string(), Value::String(value.to_string())),
            ]
            .into_iter()
            .collect(),
        );
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn test_parse() {
        let json = r#"{"signatures":[{"keyid":"c25d24c04760b6982de77736776edc6600d5f8e1e84d0bba2a7299959ce7d47f","sig":"8cd70318ea1b34c91bf7303e9c8811df43d1b4746aa9adf1d503ebb0241e0fbff9be28f36dac0318825782bf05dbbcea7171eb0ca9a89be3b02666f0f3c84301"}],"signed":{"_type":"link","name":"rebuild spytrap-adb_0.3.5-1_amd64.deb","materials":{"rust-spytrap-adb_0.3.5-1_amd64.buildinfo":{"sha512":"d130dbdbd51480f5cb79c1e6ce09fa61a69766e56725543b9c19bee8248306b2c3c2a2c66b250992bf20b2f5af7cf03bf401255104714bc9d654126fb41bc59f","sha256":"9df2f9a721f5016874c5f78ae88d3df77f9e49ea6070f935bfeeb438cd73a158"}},"products":{"spytrap-adb_0.3.5-1_amd64.deb":{"sha256":"58a7d451d5d59fda6284a05418b99e34fab32d07e63d0b164404eaaed1317edd","sha512":"f38806536701138cb1b2059565e5f73ec07288f9a3013ba986e33d510432e183e7bfe94af31bb8d480b85c84f4c145ed5c28c5949d618a4e94b2c7aecb309642"}},"environment":null,"byproducts":{},"command":[]}}"#;
        let metablock = Attestation::parse(json.as_bytes()).unwrap();
        assert_eq!(metablock, Attestation {
            metablock: Metablock {
                signatures: vec![signature(
                    "c25d24c04760b6982de77736776edc6600d5f8e1e84d0bba2a7299959ce7d47f",
                    "8cd70318ea1b34c91bf7303e9c8811df43d1b4746aa9adf1d503ebb0241e0fbff9be28f36dac0318825782bf05dbbcea7171eb0ca9a89be3b02666f0f3c84301",
                )],
                metadata: MetadataWrapper::Link(LinkMetadata {
                    name: "rebuild spytrap-adb_0.3.5-1_amd64.deb".to_string(),
                    materials: [
                        (VirtualTargetPath::new("rust-spytrap-adb_0.3.5-1_amd64.buildinfo".to_string()).unwrap(), [
                            (HashAlgorithm::Sha512, hashvalue_from_hex("d130dbdbd51480f5cb79c1e6ce09fa61a69766e56725543b9c19bee8248306b2c3c2a2c66b250992bf20b2f5af7cf03bf401255104714bc9d654126fb41bc59f").unwrap()),
                            (HashAlgorithm::Sha256, hashvalue_from_hex("9df2f9a721f5016874c5f78ae88d3df77f9e49ea6070f935bfeeb438cd73a158").unwrap()),
                        ].into_iter().collect()),
                    ].into_iter().collect(),
                    products: [
                        (VirtualTargetPath::new("spytrap-adb_0.3.5-1_amd64.deb".to_string()).unwrap(), [
                            (HashAlgorithm::Sha256, hashvalue_from_hex("58a7d451d5d59fda6284a05418b99e34fab32d07e63d0b164404eaaed1317edd").unwrap()),
                            (HashAlgorithm::Sha512, hashvalue_from_hex("f38806536701138cb1b2059565e5f73ec07288f9a3013ba986e33d510432e183e7bfe94af31bb8d480b85c84f4c145ed5c28c5949d618a4e94b2c7aecb309642").unwrap()),
                        ].into_iter().collect()),
                    ].into_iter().collect(),
                    env: None,
                    byproducts: Default::default(),
                    command: vec![].into(),
                })
            }
        });
    }

    #[test]
    fn test_append_signature() {
        // generate keypair
        let privkey = PrivateKey::new(KeyType::Ed25519).unwrap();
        let privkey = PrivateKey::from_pkcs8(&privkey, SignatureScheme::Ed25519).unwrap();
        let pubkey = privkey.public();

        // take a metablock
        let json = r#"{"signatures":[{"keyid":"c25d24c04760b6982de77736776edc6600d5f8e1e84d0bba2a7299959ce7d47f","sig":"8cd70318ea1b34c91bf7303e9c8811df43d1b4746aa9adf1d503ebb0241e0fbff9be28f36dac0318825782bf05dbbcea7171eb0ca9a89be3b02666f0f3c84301"}],"signed":{"_type":"link","name":"rebuild spytrap-adb_0.3.5-1_amd64.deb","materials":{"rust-spytrap-adb_0.3.5-1_amd64.buildinfo":{"sha512":"d130dbdbd51480f5cb79c1e6ce09fa61a69766e56725543b9c19bee8248306b2c3c2a2c66b250992bf20b2f5af7cf03bf401255104714bc9d654126fb41bc59f","sha256":"9df2f9a721f5016874c5f78ae88d3df77f9e49ea6070f935bfeeb438cd73a158"}},"products":{"spytrap-adb_0.3.5-1_amd64.deb":{"sha256":"58a7d451d5d59fda6284a05418b99e34fab32d07e63d0b164404eaaed1317edd","sha512":"f38806536701138cb1b2059565e5f73ec07288f9a3013ba986e33d510432e183e7bfe94af31bb8d480b85c84f4c145ed5c28c5949d618a4e94b2c7aecb309642"}},"environment":null,"byproducts":{},"command":[]}}"#;
        let mut attestation = Attestation::parse(json.as_bytes()).unwrap();

        // ensure it's not valid yet
        attestation.verify(1, [pubkey]).unwrap_err();

        // append a signature with our key
        attestation.sign(&privkey).unwrap();

        // ensure it's valid now
        attestation.verify(1, [pubkey]).unwrap();
    }
}
