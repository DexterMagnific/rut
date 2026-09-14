use rut::{TestResult, TestRunnerInternal, suite};

suite! {
    typename = DiagnosticReportSuite;
    name = "Diagnostic report";

    test_case(name = "explicit failure") {
        test(name = "records the failure call site") {
            TestResult::failed("expected value")
        }
    }

    test_case(name = "panic failure") {
        test(name = "records the panic trace") {
            panic!("unexpected panic")
        }
    }
}
