use crate::error::RutError;
use regex::Regex;

pub fn extract_typename(content: &str) -> Result<String, RutError> {
    // Match: typename = OptionalVisibility TypeName;
    // Examples:
    //   typename = CalculatorSuite;
    //   typename = pub CalculatorSuite;
    //   typename = pub(crate) CalculatorSuite;
    let re = Regex::new(r#"typename\s*=\s*(?:pub(?:\([^)]+\))?\s+)?(\w+)\s*;"#)
        .map_err(|e| RutError::ParseError(e.to_string()))?;

    let captures = re.captures(content).ok_or(RutError::TypenameNotFound)?;
    Ok(captures[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_typename_simple() {
        let content = r#"
suite! {
    typename = CalculatorSuite;
    name = "Calculator";
    test_case(name = "addition") {
        test(name = "adds") { TestResult::passed() }
    }
}
"#;
        assert_eq!(extract_typename(content).unwrap(), "CalculatorSuite");
    }

    #[test]
    fn test_extract_typename_pub() {
        let content = r#"
suite! {
    typename = pub CalculatorSuite;
    name = "Calculator";
    test_case(name = "addition") {
        test(name = "adds") { TestResult::passed() }
    }
}
"#;
        assert_eq!(extract_typename(content).unwrap(), "CalculatorSuite");
    }

    #[test]
    fn test_extract_typename_pub_crate() {
        let content = r#"
suite! {
    typename = pub(crate) CalculatorSuite;
    name = "Calculator";
    test_case(name = "addition") {
        test(name = "adds") { TestResult::passed() }
    }
}
"#;
        assert_eq!(extract_typename(content).unwrap(), "CalculatorSuite");
    }
}