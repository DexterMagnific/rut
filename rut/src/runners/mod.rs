pub mod parallel;
pub mod sequential;

pub use parallel::{ParallelRunner, ParallelRunnerBuilder};
pub use sequential::SequentialRunner;