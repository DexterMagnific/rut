use crate::error::RutError;
use crate::parser::{SuiteDefinition, extract_suites};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, PartialEq, Eq)]
pub struct DiscoveredSuiteFile {
    pub path: PathBuf,
    pub suites: Vec<SuiteDefinition>,
}

pub fn discover_suites(path: Option<PathBuf>) -> Result<Vec<DiscoveredSuiteFile>, RutError> {
    let start = path.unwrap_or_else(|| PathBuf::from("."));

    if start.is_file() {
        return discover_files(&start, std::iter::once(start.clone()));
    }

    let entries = WalkDir::new(&start)
        .into_iter()
        .filter_entry(|entry| !is_excluded_directory(entry.path()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"));

    discover_files(&start, entries)
}

fn discover_files(
    start: &Path,
    paths: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<DiscoveredSuiteFile>, RutError> {
    let mut discovered = Vec::new();

    for path in paths {
        let content = std::fs::read_to_string(&path).map_err(|error| {
            RutError::ParseError(format!("failed to read {}: {error}", path.display()))
        })?;
        let suites = extract_suites(&content)?;
        if !suites.is_empty() {
            discovered.push(DiscoveredSuiteFile { path, suites });
        }
    }

    if discovered.is_empty() {
        return Err(RutError::NoSuiteFileFound(start.to_path_buf()));
    }

    discovered.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(discovered)
}

fn is_excluded_directory(path: &Path) -> bool {
    path.is_dir()
        && path
            .file_name()
            .is_some_and(|name| name == "target" || name == ".git")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(root: &Path, relative_path: &str, content: &str) -> PathBuf {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn discovers_parsed_suites_in_sorted_file_order() {
        let project = TempDir::new().unwrap();
        let last = write_file(
            project.path(),
            "z_suite.rs",
            r#"suite! { typename = LastSuite; name = "Last"; }"#,
        );
        let first = write_file(
            project.path(),
            "a_suite.rs",
            r#"suite! { typename = FirstSuite; name = "First"; }"#,
        );
        let nested = write_file(
            project.path(),
            "nested/m_suite.rs",
            r#"
suite! { typename = MiddleSuite; name = "Middle"; }
suite! {
    typename = AnotherSuite;
    name = "Another";
    test_case(name = "one") {
        test(name = "first") {}
        test(name = "second") {}
    }
}
"#,
        );
        write_file(project.path(), "not_a_suite.rs", "fn main() {}");
        write_file(
            project.path(),
            "target/generated.rs",
            r#"suite! { typename = TargetSuite; name = "Target"; }"#,
        );
        write_file(
            project.path(),
            ".git/old.rs",
            r#"suite! { typename = GitSuite; name = "Git"; }"#,
        );

        let discovered = discover_suites(Some(project.path().to_path_buf())).unwrap();

        assert_eq!(
            discovered.iter().map(|file| &file.path).collect::<Vec<_>>(),
            [&first, &nested, &last]
        );
        assert_eq!(discovered[1].suites.len(), 2);
        assert_eq!(discovered[1].suites[1].typename, "AnotherSuite");
        assert_eq!(discovered[1].suites[1].cases[0].name, "one");
        assert_eq!(discovered[1].suites[1].cases[0].test_count, 2);
    }

    #[test]
    fn discovers_a_direct_suite_file() {
        let project = TempDir::new().unwrap();
        let suite_file = write_file(
            project.path(),
            "suite.rs",
            r#"suite! { typename = DirectSuite; name = "Direct"; }"#,
        );

        let discovered = discover_suites(Some(suite_file.clone())).unwrap();

        assert_eq!(discovered[0].path, suite_file);
        assert_eq!(discovered[0].suites[0].name, "Direct");
    }

    #[test]
    fn ignores_suite_text_in_strings_and_comments() {
        let project = TempDir::new().unwrap();
        write_file(
            project.path(),
            "false_positive.rs",
            r#"
const EXAMPLE: &str = "suite! { typename = FakeSuite; name = fake; }";
// suite! { typename = CommentSuite; name = "Comment"; }
"#,
        );

        let error = discover_suites(Some(project.path().to_path_buf())).unwrap_err();

        assert!(matches!(error, RutError::NoSuiteFileFound(_)));
    }

    #[test]
    fn reports_when_no_suite_files_are_found() {
        let project = TempDir::new().unwrap();
        write_file(project.path(), "plain.rs", "fn main() {}");

        let error = discover_suites(Some(project.path().to_path_buf())).unwrap_err();

        assert!(matches!(error, RutError::NoSuiteFileFound(_)));
    }
}
