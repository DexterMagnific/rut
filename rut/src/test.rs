use crate::report::{BoxFuture, SourceLocation, TestContext, TestResult};
use async_trait::async_trait;
use std::time::Duration;

pub trait TestInternal: Send + Sync {
    fn name(&self) -> &str;
    fn source_location(&self) -> Option<SourceLocation>;
    fn timeout(&self) -> Option<Duration> {
        None
    }
    fn retries(&self) -> Option<u32> {
        None
    }
    fn properties(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    fn run<'a>(&'a self, ctx: Option<&'a TestContext>) -> BoxFuture<'a, TestResult>;
}

/// A single executable test belonging to a [`crate::TestCase`].
///
/// Implement this trait when constructing suites manually. The [`crate::suite!`]
/// macro generates the implementation for declarative tests. Implementations
/// must be thread-safe because a test may run inside a spawned task when its
/// case is executed by [`crate::ParallelRunner`].
///
/// The test name must remain stable and should match the name reported by the
/// returned [`TestResult`]. The default metadata methods are sufficient for a
/// basic test; override them to provide source information, a cooperative
/// timeout, retry attempts, or static properties.
///
/// `run` receives the optional suite context and may perform asynchronous
/// work. Returning [`TestResult::failed`] marks the test as failed; returning
/// [`TestResult::skipped`] excludes it from the failure count.
#[async_trait]
pub trait Test: Send + Sync {
    fn name(&self) -> &str;
    fn source_location(&self) -> Option<SourceLocation> {
        None
    }
    fn timeout(&self) -> Option<Duration> {
        None
    }
    fn retries(&self) -> Option<u32> {
        None
    }
    fn properties(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    async fn run(&self, ctx: Option<&TestContext>) -> TestResult;
}

/// Blanket implementation: converts external Test to internal TestInternal
impl<T: Test + ?Sized> TestInternal for T {
    fn name(&self) -> &str {
        Test::name(self)
    }

    fn source_location(&self) -> Option<SourceLocation> {
        Test::source_location(self)
    }

    fn timeout(&self) -> Option<Duration> {
        Test::timeout(self)
    }

    fn retries(&self) -> Option<u32> {
        Test::retries(self)
    }

    fn properties(&self) -> Vec<(String, String)> {
        Test::properties(self)
    }

    fn run<'a>(&'a self, ctx: Option<&'a TestContext>) -> BoxFuture<'a, TestResult> {
        Box::pin(async move { self.run(ctx).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ManualTest;

    #[async_trait]
    impl Test for ManualTest {
        fn name(&self) -> &str {
            "manual"
        }

        async fn run(&self, _ctx: Option<&TestContext>) -> TestResult {
            TestResult::passed()
        }
    }

    #[test]
    fn manual_tests_default_to_no_source_location() {
        assert_eq!(Test::source_location(&ManualTest), None);
    }
}
