use crate::report::{BoxFuture, SourceLocation, TestContext, TestResult};
use async_trait::async_trait;
use std::time::Duration;

pub trait TestInternal: Send + Sync {
    fn name(&self) -> &str;
    fn source_location(&self) -> Option<SourceLocation>;
    fn timeout(&self) -> Option<Duration> {
        None
    }
    fn run<'a>(&'a self, ctx: Option<&'a TestContext>) -> BoxFuture<'a, TestResult>;
}

/// User-facing trait - implement this in your test code
/// Uses async_trait for clean async fn syntax
#[async_trait]
pub trait Test: Send + Sync {
    fn name(&self) -> &str;
    fn source_location(&self) -> Option<SourceLocation> {
        None
    }
    fn timeout(&self) -> Option<Duration> {
        None
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
