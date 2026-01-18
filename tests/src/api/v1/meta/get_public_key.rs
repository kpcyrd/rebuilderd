use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use rebuilderd::attestation::pubkey_to_pem;
use rebuilderd_common::api::v1::MetaRestApi;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_valid_key(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let mut results = client.get_public_keys().await.unwrap().current;

    assert_eq!(1, results.len());

    let result = results.pop().unwrap();
    let key_as_pem = pubkey_to_pem(&isolated_server.public_key).unwrap();

    assert_eq!(key_as_pem, result);
}
