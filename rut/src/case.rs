use crate::report::{BoxFuture, SuiteArgs, TestContext};
use crate::test::{Test, TestInternal};
use async_trait::async_trait;

pub trait TestCaseInternal: Send + Sync + std::panic::UnwindSafe {
    fn name(&self) -> &str;
    fn setup_case<'a>(
        &'a mut self,
        _ctx: Option<&'a TestContext>,
        _args: &'a SuiteArgs,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
    fn teardown_case<'a>(
        &'a mut self,
        _ctx: Option<&'a TestContext>,
        _args: &'a SuiteArgs,
    ) -> BoxFuture<'a, ()> {
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

/// A group of tests with optional setup and teardown hooks.
///
/// Implement this trait to build a case manually. The [`crate::suite!`] macro
/// generates cases from `test_case` blocks. Cases are cloned before execution,
/// so [`clone_box`](Self::clone_box) must return an independent equivalent
/// value. Case tests run sequentially, even when the enclosing suite uses
/// [`crate::ParallelRunner`].
///
/// The hooks receive the optional suite [`crate::TestContext`] and the suite
/// arguments. They are asynchronous and default to no-ops. Returning tests from
/// [`tests`](Self::tests) determines the case's execution order.
#[async_trait]
pub trait TestCase: Send + Sync + std::panic::UnwindSafe {
    fn name(&self) -> &str;
    async fn setup_case(&mut self, _ctx: Option<&TestContext>, _args: &SuiteArgs) {}
    async fn teardown_case(&mut self, _ctx: Option<&TestContext>, _args: &SuiteArgs) {}
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

    fn setup_case<'a>(
        &'a mut self,
        ctx: Option<&'a TestContext>,
        args: &'a SuiteArgs,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.setup_case(ctx, args).await })
    }

    fn teardown_case<'a>(
        &'a mut self,
        ctx: Option<&'a TestContext>,
        args: &'a SuiteArgs,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.teardown_case(ctx, args).await })
    }

    fn tests(&self) -> Vec<Box<dyn TestInternal>> {
        TestCase::tests(self)
            .into_iter()
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
