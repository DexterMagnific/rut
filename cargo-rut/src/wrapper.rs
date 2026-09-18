use crate::plugins::PluginCrate;
use std::path::Path;

pub struct WrapperOptions<'a> {
    pub plugin_crates: &'a [PluginCrate],
}

/// Generates the `main.rs` of the temporary crate that runs one suite.
///
/// The harness owns argument parsing, so the generated code depends only on the
/// suite typename and the plugin crates to register; command line options never
/// appear here and therefore never trigger a rebuild.
pub fn generate_wrapper(suite_file: &Path, typename: &str, options: WrapperOptions<'_>) -> String {
    // Convert to absolute path so include! can find it from temp directory
    let absolute_suite_path = std::fs::canonicalize(suite_file)
        .unwrap_or_else(|_| suite_file.to_path_buf())
        .display()
        .to_string();

    let registrations = options
        .plugin_crates
        .iter()
        .map(|plugin| format!("    {}::__rut_plugins(&mut registry);\n", plugin.ident()))
        .collect::<String>();

    format!(
        r#"include!(r"{absolute_suite_path}");

#[tokio::main]
async fn main() -> std::process::ExitCode {{
    #[allow(unused_mut)]
    let mut registry = rut::cli::PluginRegistry::with_builtins();
{registrations}    rut::cli::harness_main(registry, || Box::new({typename}::new())).await
}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn wrapper(plugin_crates: &[PluginCrate]) -> String {
        generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            WrapperOptions { plugin_crates },
        )
    }

    #[test]
    fn generates_a_harness_entry_point() {
        let wrapper = wrapper(&[]);

        assert!(wrapper.contains("rut::cli::PluginRegistry::with_builtins()"));
        assert!(wrapper.contains("Box::new(CalculatorSuite::new())"));
        assert!(!wrapper.contains("__rut_plugins"));
    }

    #[test]
    fn does_not_bake_command_line_options_into_the_source() {
        let wrapper = wrapper(&[]);

        assert!(!wrapper.contains("with_max_jobs"));
        assert!(!wrapper.contains("shuffle_test_cases"));
        assert!(!wrapper.contains("with_filter"));
        assert!(!wrapper.contains("JUnitReporter"));
    }

    #[test]
    fn registers_every_plugin_crate() {
        let wrapper = wrapper(&[
            PluginCrate {
                package: "my-plugins".to_string(),
                path: PathBuf::from("/tmp/my-plugins"),
            },
            PluginCrate {
                package: "other".to_string(),
                path: PathBuf::from("/tmp/other"),
            },
        ]);

        assert!(wrapper.contains("my_plugins::__rut_plugins(&mut registry);"));
        assert!(wrapper.contains("other::__rut_plugins(&mut registry);"));
    }
}
