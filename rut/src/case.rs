use async_trait::async_trait;
use crate::test::{Test, TestInternal};
use crate::report::{TestContext, BoxFuture};

pub trait TestCaseInternal: Send + Sync + std::panic::UnwindSafe {
    fn name(&self) -> &str;
    fn setup_case<'a>(&'a mut self, _ctx: Option<&'a TestContext>) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
    fn teardown_case<'a>(&'a mut self, _ctx: Option<&'a TestContext>) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
    fn tests(&self) -> Vec<Box<dyn TestInternal>>;
    fn clone_box(&self) -> Box<dyn TestCaseInternal>;
}

impl Clone for Box<dyn TestCaseInternal> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// User-facing trait - implement this in your test code
/// Uses async_trait for clean async fn syntax
#[async_trait]
pub trait TestCase: Send + Sync + std::panic::UnwindSafe {
    fn name(&self) -> &str;
    async fn setup_case(&mut self, _ctx: Option<&TestContext>) {}
    async fn teardown_case(&mut self, _ctx: Option<&TestContext>) {}
    fn tests(&self) -> Vec<Box<dyn Test>>;
    fn clone_box(&self) -> Box<dyn TestCase>;
}

impl Clone for Box<dyn TestCase> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Blanket implementation: converts external TestCase to internal TestCaseInternal
impl<T: TestCase + ?Sized> TestCaseInternal for T {
    fn name(&self) -> &str {
        TestCase::name(self)
    }

    fn setup_case<'a>(&'a mut self, ctx: Option<&'a TestContext>) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.setup_case(ctx).await })
    }

    fn teardown_case<'a>(&'a mut self, ctx: Option<&'a TestContext>) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.teardown_case(ctx).await })
    }

    fn tests(&self) -> Vec<Box<dyn TestInternal>> {
        TestCase::tests(self).into_iter()
            .map(|t| {
                let boxed: Box<dyn Test> = t;
                unsafe { std::mem::transmute::<Box<dyn Test>, Box<dyn TestInternal>>(boxed) }
            })
            .collect()
    }

    fn clone_box(&self) -> Box<dyn TestCaseInternal> {
        let boxed: Box<dyn TestCase> = TestCase::clone_box(self);
        unsafe { std::mem::transmute::<Box<dyn TestCase>, Box<dyn TestCaseInternal>>(boxed) }
    }
}
