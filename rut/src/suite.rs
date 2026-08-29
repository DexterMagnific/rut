use async_trait::async_trait;
use crate::case::{TestCase, TestCaseInternal};
use crate::report::{TestContext, BoxFuture};

pub trait TestSuiteInternal: Send + Sync {
    fn setup_suite<'a>(&'a mut self) -> BoxFuture<'a, ()>;

    fn context(&self) -> Option<&TestContext>;
    fn context_mut(&mut self) -> &mut Option<TestContext>;

    fn teardown_suite<'a>(&'a mut self) -> BoxFuture<'a, ()>;

    fn name(&self) -> &str;
    fn test_cases(&self) -> Vec<Box<dyn TestCaseInternal>>;
    fn test_cases_mut(&mut self) -> &mut Vec<Box<dyn TestCaseInternal>>;
    fn clone_box(&self) -> Box<dyn TestSuiteInternal>;
}

impl Clone for Box<dyn TestSuiteInternal> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// User-facing trait - implement this in your test code
/// Uses async_trait for clean async fn syntax
#[async_trait]
pub trait TestSuite: Send + Sync {
    async fn setup_suite(&mut self);

    fn context(&self) -> Option<&TestContext>;
    fn context_mut(&mut self) -> &mut Option<TestContext>;

    async fn teardown_suite(&mut self);

    fn name(&self) -> &str;
    fn test_cases(&self) -> Vec<Box<dyn TestCase>>;
    fn test_cases_mut(&mut self) -> &mut Vec<Box<dyn TestCase>>;
}

/// Blanket implementation: converts external TestSuite to internal TestSuiteInternal
impl<T: TestSuite + Clone + Sized + 'static> TestSuiteInternal for T {
    fn setup_suite<'a>(&'a mut self) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.setup_suite().await })
    }

    fn context(&self) -> Option<&TestContext> {
        TestSuite::context(self)
    }

    fn context_mut(&mut self) -> &mut Option<TestContext> {
        TestSuite::context_mut(self)
    }

    fn teardown_suite<'a>(&'a mut self) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.teardown_suite().await })
    }

    fn name(&self) -> &str {
        TestSuite::name(self)
    }

    fn test_cases(&self) -> Vec<Box<dyn TestCaseInternal>> {
        TestSuite::test_cases(self).into_iter()
            .map(|c| {
                let boxed: Box<dyn TestCase> = c;
                unsafe { std::mem::transmute::<Box<dyn TestCase>, Box<dyn TestCaseInternal>>(boxed) }
            })
            .collect()
    }

    fn test_cases_mut(&mut self) -> &mut Vec<Box<dyn TestCaseInternal>> {
        unsafe { std::mem::transmute(TestSuite::test_cases_mut(self)) }
    }

    fn clone_box(&self) -> Box<dyn TestSuiteInternal> {
        // Box the concrete type and coerce to trait object
        // This creates a fat pointer (vtable + data pointer)
        Box::new(self.clone()) as Box<dyn TestSuiteInternal>
    }
}



pub struct TestSuiteBuilder<S: TestSuiteInternal> {
    suite: S,
}

impl<S: TestSuiteInternal> TestSuiteBuilder<S> {
    pub fn new(suite: S) -> Self {
        Self { suite }
    }

    pub fn add_test_case(mut self, case: Box<dyn TestCaseInternal>) -> Self {
        self.suite.test_cases_mut().push(case);
        self
    }

    pub fn build(self) -> S {
        self.suite
    }
}
