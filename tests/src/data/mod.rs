mod build_reports;
mod job_requests;
mod package_reports;

pub use build_reports::*;
use in_toto::crypto::{KeyType, PrivateKey, SignatureScheme};
use in_toto::runlib::in_toto_run;
pub use job_requests::*;
pub use package_reports::*;
use std::os::unix;
use tempfile::TempDir;

pub const DUMMY_DISTRIBUTION: &str = "distribution";
pub const DUMMY_OTHER_DISTRIBUTION: &str = "other-distribution";
pub const DUMMY_RELEASE: &str = "release";
pub const DUMMY_OTHER_RELEASE: &str = "other-release";
pub const DUMMY_COMPONENT: &str = "component";
pub const DUMMY_OTHER_COMPONENT: &str = "other-component";
pub const DUMMY_ARCHITECTURE: &str = "architecture";
pub const DUMMY_OTHER_ARCHITECTURE: &str = "other-architecture";

// TODO: rebuilderd assumes the distribution is the backend
pub const DUMMY_BACKEND: &str = DUMMY_DISTRIBUTION;
pub const DUMMY_OTHER_BACKEND: &str = DUMMY_OTHER_DISTRIBUTION;
pub const DUMMY_WORKER: &str = "worker";
pub const DUMMY_OTHER_WORKER: &str = "other-worker";

pub fn create_dummy_signed_attestation(input_name: &str, output_name: &str) -> String {
    let private_key = PrivateKey::from_pkcs8(
        PrivateKey::new(KeyType::Ed25519).unwrap().as_slice(),
        SignatureScheme::Ed25519,
    )
    .unwrap();

    let temp_dir = TempDir::new().unwrap();

    let input_path = temp_dir.path().join(input_name);
    let output_path = temp_dir.path().join(output_name);

    unix::fs::symlink("/dev/null", &input_path).unwrap();
    unix::fs::symlink("/dev/null", &output_path).unwrap();

    let signature = in_toto_run(
        &format!("rebuild {}", output_name),
        Some(temp_dir.path().to_str().unwrap()),
        &[input_path.to_str().unwrap()],
        &[output_path.to_str().unwrap()],
        &[],
        Some(&private_key),
        Some(&["sha256", "sha512"]),
        Some(&[temp_dir.path().to_str().unwrap()]),
    )
    .unwrap();

    serde_json::to_string(&signature).unwrap()
}

pub fn create_dummy_unsigned_attestation(input_name: &str, output_name: &str) -> String {
    let temp_dir = TempDir::new().unwrap();

    let input_path = temp_dir.path().join(input_name);
    let output_path = temp_dir.path().join(output_name);

    unix::fs::symlink("/dev/null", &input_path).unwrap();
    unix::fs::symlink("/dev/null", &output_path).unwrap();

    let signature = in_toto_run(
        &format!("rebuild {}", output_name),
        Some(temp_dir.path().to_str().unwrap()),
        &[input_path.to_str().unwrap()],
        &[output_path.to_str().unwrap()],
        &[],
        None,
        Some(&["sha256", "sha512"]),
        Some(&[temp_dir.path().to_str().unwrap()]),
    )
    .unwrap();

    serde_json::to_string(&signature).unwrap()
}
