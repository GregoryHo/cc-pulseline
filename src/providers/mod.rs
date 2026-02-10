pub mod env;
pub mod git;
pub mod stdin;
pub mod transcript;

pub use env::{EnvCollector, EnvSnapshot, FileSystemEnvCollector, StubEnvCollector};
pub use git::{GitCollector, GitSnapshot, LocalGitCollector, StubGitCollector};
pub use stdin::{StdinCollector, StdinSnapshot, StubStdinCollector};
pub use transcript::{
    FileTranscriptCollector, StubTranscriptCollector, TranscriptCollector, TranscriptSnapshot,
};
