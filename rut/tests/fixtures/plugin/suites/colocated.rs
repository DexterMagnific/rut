// Lives in the same crate as the plugins, and uses one of them.

use rut::{TestResult, suite};
use rut_example_plugins::summary_label;

suite! {
    typename = ColocatedSuite;
    name = "colocated suite";

    test_case(name = "plugin crate items") {
        test(name = "are reachable from the suite") {
            if summary_label() == "summary" {
                TestResult::passed()
            } else {
                TestResult::failed("unexpected default label")
            }
        }

        test(name = "sees forwarded suite arguments") {
            match args.get("mode") {
                Some("e2e") => TestResult::passed(),
                Some(other) => TestResult::failed(format!("unexpected mode '{other}'")),
                None => TestResult::skipped("no mode argument supplied"),
            }
        }
    }
}
