use crate::data::{DUMMY_ARCHITECTURE, DUMMY_BACKEND};
use rebuilderd_common::api::v1::PopQueuedJobRequest;

pub fn job_request() -> PopQueuedJobRequest {
    PopQueuedJobRequest {
        supported_backends: vec![DUMMY_BACKEND.to_string()],
        architecture: DUMMY_ARCHITECTURE.to_string(),
        supported_architectures: vec![DUMMY_ARCHITECTURE.to_string()],
    }
}
