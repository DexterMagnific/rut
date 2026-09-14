use async_trait::async_trait;
use crate::report::{TestResult, TestContext, BoxFuture};

pub trait TestInternal: Send + Sync {
    fn name(&self) -> &str;
    fn run<'a>(&'a self, ctx: Option<&'a TestContext>) -> BoxFuture<'a, TestResult>;
}

/// User-facing trait - implement this in your test code
/// Uses async_trait for clean async fn syntax
#[async_trait]
pub trait Test: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: Option<&TestContext>) -> TestResult;
}

/// Blanket implementation: converts external Test to internal TestInternal
impl<T: Test + ?Sized> TestInternal for T {
    fn name(&self) -> &str {
        Test::name(self)
    }

    fn run<'a>(&'a self, ctx: Option<&'a TestContext>) -> BoxFuture<'a, TestResult> {
        Box::pin(async move { self.run(ctx).await })
    }
}
