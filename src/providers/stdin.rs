use crate::types::StdinPayload;

#[derive(Debug, Clone, Default)]
pub struct StdinSnapshot {
    pub payload: Option<StdinPayload>,
}

pub trait StdinCollector {
    fn collect_stdin(&self, payload: &StdinPayload) -> StdinSnapshot;
}

#[derive(Debug, Default)]
pub struct StubStdinCollector;

impl StdinCollector for StubStdinCollector {
    fn collect_stdin(&self, payload: &StdinPayload) -> StdinSnapshot {
        StdinSnapshot {
            payload: Some(payload.clone()),
        }
    }
}
