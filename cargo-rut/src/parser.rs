use crate::error::RutError;
use syn::parse::{Parse, ParseStream};
use syn::visit::Visit;
use syn::{Block, Ident, Item, LitStr, Token, Type, Visibility, braced, parenthesized};

#[derive(Debug, PartialEq, Eq)]
pub struct CaseDefinition {
    pub name: String,
    pub test_count: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SuiteDefinition {
    pub name: String,
    pub typename: String,
    pub cases: Vec<CaseDefinition>,
}

struct SuiteDeclaration {
    definition: SuiteDefinition,
}

struct NameArgument {
    name: LitStr,
}

impl Parse for NameArgument {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "name" {
            return Err(syn::Error::new(key.span(), "expected `name`"));
        }
        input.parse::<Token![=]>()?;
        let name = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("unexpected argument"));
        }
        Ok(Self { name })
    }
}

fn parse_dsl_block(input: ParseStream<'_>) -> syn::Result<Block> {
    if input.peek(Token![async]) {
        input.parse::<Token![async]>()?;
    }
    input.parse()
}

fn parse_test_arguments(input: ParseStream<'_>) -> syn::Result<()> {
    while !input.is_empty() {
        input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        input.parse::<LitStr>()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(())
}

fn parse_case(input: ParseStream<'_>) -> syn::Result<CaseDefinition> {
    let arguments;
    parenthesized!(arguments in input);
    let name = arguments.parse::<NameArgument>()?.name.value();
    let body;
    braced!(body in input);
    let mut test_count = 0;

    while !body.is_empty() {
        if body.peek(Ident) {
            let fork = body.fork();
            let keyword: Ident = fork.parse()?;
            match keyword.to_string().as_str() {
                "setup" | "teardown" => {
                    body.parse::<Ident>()?;
                    parse_dsl_block(&body)?;
                    continue;
                }
                "test" if fork.peek(syn::token::Paren) => {
                    body.parse::<Ident>()?;
                    let arguments;
                    parenthesized!(arguments in body);
                    parse_test_arguments(&arguments)?;
                    parse_dsl_block(&body)?;
                    test_count += 1;
                    continue;
                }
                _ => {}
            }
        }

        body.parse::<Item>()?;
    }

    Ok(CaseDefinition { name, test_count })
}

impl Parse for SuiteDeclaration {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let typename_key: Ident = input.parse()?;
        if typename_key != "typename" {
            return Err(syn::Error::new(typename_key.span(), "expected `typename`"));
        }
        input.parse::<Token![=]>()?;
        input.parse::<Visibility>()?;
        let typename: Ident = input.parse()?;
        input.parse::<Token![;]>()?;

        let name_key: Ident = input.parse()?;
        if name_key != "name" {
            return Err(syn::Error::new(name_key.span(), "expected `name`"));
        }
        input.parse::<Token![=]>()?;
        let name: LitStr = input.parse()?;
        input.parse::<Token![;]>()?;

        let mut cases = Vec::new();
        while !input.is_empty() {
            if input.peek(Ident) {
                let fork = input.fork();
                let keyword: Ident = fork.parse()?;
                match keyword.to_string().as_str() {
                    "context" => {
                        input.parse::<Ident>()?;
                        input.parse::<Token![=]>()?;
                        input.parse::<Type>()?;
                        input.parse::<Token![;]>()?;
                        continue;
                    }
                    "setup" | "teardown" => {
                        input.parse::<Ident>()?;
                        parse_dsl_block(input)?;
                        continue;
                    }
                    "test_case" if fork.peek(syn::token::Paren) => {
                        input.parse::<Ident>()?;
                        cases.push(parse_case(input)?);
                        continue;
                    }
                    _ => {}
                }
            }

            input.parse::<Item>()?;
        }

        Ok(Self {
            definition: SuiteDefinition {
                name: name.value(),
                typename: typename.to_string(),
                cases,
            },
        })
    }
}

#[derive(Default)]
struct SuiteVisitor {
    definitions: Vec<SuiteDefinition>,
    error: Option<syn::Error>,
}

impl<'ast> Visit<'ast> for SuiteVisitor {
    fn visit_macro(&mut self, suite_macro: &'ast syn::Macro) {
        let is_suite = suite_macro
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "suite");

        if is_suite && self.error.is_none() {
            match syn::parse2::<SuiteDeclaration>(suite_macro.tokens.clone()) {
                Ok(declaration) => self.definitions.push(declaration.definition),
                Err(error) => self.error = Some(error),
            }
        }

        syn::visit::visit_macro(self, suite_macro);
    }
}

pub fn extract_suites(content: &str) -> Result<Vec<SuiteDefinition>, RutError> {
    let syntax =
        syn::parse_file(content).map_err(|error| RutError::ParseError(error.to_string()))?;
    let mut visitor = SuiteVisitor::default();
    visitor.visit_file(&syntax);

    if let Some(error) = visitor.error {
        return Err(RutError::ParseError(error.to_string()));
    }

    Ok(visitor.definitions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_suite_names_and_typenames_from_multiple_macros() {
        let content = r##"
suite! {
    typename = FirstSuite;
    name = "First suite";
    test_case(name = "not the suite name") {}
}

mod nested {
    rut::suite! {
        typename = pub(crate) SecondSuite;
        name = r#"Second suite"#;
        test_case(name = "case") {}
    }
}
"##;

        assert_eq!(
            extract_suites(content).unwrap(),
            [
                SuiteDefinition {
                    name: "First suite".to_string(),
                    typename: "FirstSuite".to_string(),
                    cases: vec![CaseDefinition {
                        name: "not the suite name".to_string(),
                        test_count: 0,
                    }],
                },
                SuiteDefinition {
                    name: "Second suite".to_string(),
                    typename: "SecondSuite".to_string(),
                    cases: vec![CaseDefinition {
                        name: "case".to_string(),
                        test_count: 0,
                    }],
                },
            ]
        );
    }

    #[test]
    fn extracts_ordered_cases_and_counts_top_level_tests() {
        let content = r#"
suite! {
    typename = CountedSuite;
    name = "Counted";
    context = crate::Context<String>;
    struct SuiteHelper;
    setup async {}
    teardown {}

    test_case(name = "empty") {
        struct CaseHelper;
        setup {}
        teardown async {}
    }

    test_case(name = "several") {
        fn test() {}
        test(name = "first") {}
        test(name = "second", category = "fast") async {}
    }
}
"#;

        assert_eq!(
            extract_suites(content).unwrap()[0].cases,
            [
                CaseDefinition {
                    name: "empty".to_string(),
                    test_count: 0,
                },
                CaseDefinition {
                    name: "several".to_string(),
                    test_count: 2,
                },
            ]
        );
    }

    #[test]
    fn ignores_suite_text_outside_macro_invocations() {
        let content = r#"
const EXAMPLE: &str = "suite! { typename = FakeSuite; name = fake; }";
// suite! { typename = CommentSuite; name = "Comment"; }
"#;

        assert!(extract_suites(content).unwrap().is_empty());
    }
}
