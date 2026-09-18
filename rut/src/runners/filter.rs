use crate::{Test, TestCase};

pub(super) struct SelectedCase {
    pub name: String,
    pub case: Box<dyn TestCase>,
    pub tests: Vec<Box<dyn Test>>,
}

pub(super) fn select_cases(
    suite_name: &str,
    cases: Vec<Box<dyn TestCase>>,
    filters: &[String],
) -> Vec<SelectedCase> {
    cases
        .into_iter()
        .filter_map(|case| {
            let case_name = case.name().to_string();
            let tests = case
                .tests()
                .into_iter()
                .filter(|test| {
                    filters.is_empty()
                        || filters.iter().any(|filter| {
                            format!("{suite_name}.{case_name}.{}", test.name()).contains(filter)
                        })
                })
                .collect::<Vec<_>>();

            (!tests.is_empty()).then_some(SelectedCase {
                name: case_name,
                case,
                tests,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestContext, TestResult};
    use async_trait::async_trait;

    struct NamedTest(&'static str);

    #[async_trait]
    impl Test for NamedTest {
        fn name(&self) -> &str {
            self.0
        }

        async fn run(&self, _ctx: Option<&TestContext>, _args: &crate::SuiteArgs) -> TestResult {
            TestResult::passed()
        }
    }

    #[derive(Clone)]
    struct NamedCase(&'static str, Vec<&'static str>);

    #[async_trait]
    impl TestCase for NamedCase {
        fn name(&self) -> &str {
            self.0
        }

        fn tests(&self) -> Vec<Box<dyn Test>> {
            self.1
                .iter()
                .map(|name| Box::new(NamedTest(name)) as Box<dyn Test>)
                .collect()
        }

        fn clone_box(&self) -> Box<dyn TestCase> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn selects_tests_by_qualified_name_substring() {
        let cases = vec![
            Box::new(NamedCase("math", vec!["adds", "subtracts"])) as Box<dyn TestCase>,
            Box::new(NamedCase("text", vec!["joins"])) as Box<dyn TestCase>,
        ];

        let selected = select_cases("calculator", cases, &["math.add".to_string()]);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "math");
        assert_eq!(selected[0].tests.len(), 1);
        assert_eq!(selected[0].tests[0].name(), "adds");
    }
}
