use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Block, Error, Ident, Item, LitStr, Token, Type, Visibility, braced, parenthesized,
    parse_macro_input,
};

struct NameArg {
    name: LitStr,
}

impl Parse for NameArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "name" {
            return Err(Error::new(key.span(), "expected `name`"));
        }
        input.parse::<Token![=]>()?;
        let name: LitStr = input.parse()?;
        if name.value().is_empty() {
            return Err(Error::new(name.span(), "name must not be empty"));
        }
        if !input.is_empty() {
            return Err(input.error("unexpected argument"));
        }
        Ok(Self { name })
    }
}

struct DslBlock {
    block: Block,
}

impl Parse for DslBlock {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(Token![async]) {
            input.parse::<Token![async]>()?;
        }
        Ok(Self {
            block: input.parse()?,
        })
    }
}

struct TestArgs {
    name: LitStr,
    properties: Vec<(LitStr, LitStr)>,
}

impl Parse for TestArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut name = None;
        let mut properties = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            if key == "name" {
                if name.replace(value.clone()).is_some() {
                    return Err(Error::new(key.span(), "duplicate `name` argument"));
                }
                if value.value().is_empty() {
                    return Err(Error::new(value.span(), "name must not be empty"));
                }
            } else {
                properties.push((LitStr::new(&key.to_string(), key.span()), value));
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        let name = name.ok_or_else(|| input.error("missing required `name` argument"))?;
        Ok(Self { name, properties })
    }
}

struct TestDecl {
    name: LitStr,
    properties: Vec<(LitStr, LitStr)>,
    block: Block,
}

enum CaseEntry {
    Item(Box<Item>),
    Setup(Block),
    Test(TestDecl),
    Teardown(Block),
}

struct CaseDecl {
    name: LitStr,
    entries: Vec<CaseEntry>,
}

impl CaseDecl {
    fn parse_body(name: LitStr, input: ParseStream<'_>) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            if input.peek(Ident) {
                let fork = input.fork();
                let keyword: Ident = fork.parse()?;
                match keyword.to_string().as_str() {
                    "setup" => {
                        input.parse::<Ident>()?;
                        entries.push(CaseEntry::Setup(input.parse::<DslBlock>()?.block));
                        continue;
                    }
                    "teardown" => {
                        input.parse::<Ident>()?;
                        entries.push(CaseEntry::Teardown(input.parse::<DslBlock>()?.block));
                        continue;
                    }
                    "test" if fork.peek(syn::token::Paren) => {
                        input.parse::<Ident>()?;
                        let args;
                        parenthesized!(args in input);
                        let TestArgs { name, properties } = args.parse()?;
                        let block = input.parse::<DslBlock>()?.block;
                        entries.push(CaseEntry::Test(TestDecl {
                            name,
                            properties,
                            block,
                        }));
                        continue;
                    }
                    _ => {}
                }
            }

            entries.push(CaseEntry::Item(Box::new(input.parse()?)));
        }

        Ok(Self { name, entries })
    }
}

struct SuiteDecl {
    visibility: Visibility,
    type_ident: Ident,
    name: LitStr,
    context: Option<Type>,
    setup: Option<Block>,
    teardown: Option<Block>,
    cases: Vec<CaseDecl>,
    helper_items: Vec<Item>,
}

impl Parse for SuiteDecl {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "typename" {
            return Err(Error::new(key.span(), "expected `typename`"));
        }
        input.parse::<Token![=]>()?;
        let visibility: Visibility = input.parse()?;
        let type_ident: Ident = input.parse()?;
        input.parse::<Token![;]>()?;
        let name = parse_name(input)?;
        input.parse::<Token![;]>()?;

        let mut context = None;
        let mut setup = None;
        let mut teardown = None;
        let mut cases = Vec::new();
        let mut helper_items = Vec::new();
        let mut case_names = HashSet::new();

        while !input.is_empty() {
            if input.peek(Ident) {
                let fork = input.fork();
                let keyword: Ident = fork.parse()?;
                match keyword.to_string().as_str() {
                    "context" => {
                        input.parse::<Ident>()?;
                        input.parse::<Token![=]>()?;
                        let context_type: Type = input.parse()?;
                        input.parse::<Token![;]>()?;
                        set_once_at(&mut context, context_type, &keyword, "context")?;
                        continue;
                    }
                    "setup" => {
                        input.parse::<Ident>()?;
                        let block = input.parse::<DslBlock>()?.block;
                        set_once_at(&mut setup, block, &keyword, "setup")?;
                        continue;
                    }
                    "teardown" => {
                        input.parse::<Ident>()?;
                        let block = input.parse::<DslBlock>()?.block;
                        set_once_at(&mut teardown, block, &keyword, "teardown")?;
                        continue;
                    }
                    "test_case" if fork.peek(syn::token::Paren) => {
                        input.parse::<Ident>()?;
                        let args;
                        parenthesized!(args in input);
                        let case_name = args.parse::<NameArg>()?.name;
                        let body;
                        braced!(body in input);
                        if !case_names.insert(case_name.value()) {
                            return Err(Error::new_spanned(case_name, "duplicate test case name"));
                        }
                        cases.push(CaseDecl::parse_body(case_name, &body)?);
                        continue;
                    }
                    _ => {}
                }
            }

            helper_items.push(input.parse()?);
        }

        if cases.is_empty() {
            return Err(Error::new(
                type_ident.span(),
                "a test suite must contain at least one `test_case(...) { ... }` declaration",
            ));
        }

        Ok(Self {
            visibility,
            type_ident,
            name,
            context,
            setup,
            teardown,
            cases,
            helper_items,
        })
    }
}

fn parse_name(input: ParseStream<'_>) -> syn::Result<LitStr> {
    let key: Ident = input.parse()?;
    if key != "name" {
        return Err(Error::new(key.span(), "expected `name`"));
    }
    input.parse::<Token![=]>()?;
    let name: LitStr = input.parse()?;
    if name.value().is_empty() {
        return Err(Error::new(name.span(), "name must not be empty"));
    }
    Ok(name)
}

#[proc_macro]
pub fn suite(input: TokenStream) -> TokenStream {
    let declaration = parse_macro_input!(input as SuiteDecl);
    expand_suite(declaration)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_suite(declaration: SuiteDecl) -> syn::Result<proc_macro2::TokenStream> {
    let rut = rut_path();
    let SuiteDecl {
        visibility,
        type_ident,
        name,
        context,
        setup,
        teardown,
        cases,
        helper_items,
    } = declaration;
    let module_ident = format_ident!("__rut_generated_{}", type_ident);

    let mut case_factories = Vec::new();
    let mut generated_cases = Vec::new();
    for (index, case) in cases.into_iter().enumerate() {
        let case_module = format_ident!("__rut_generated_case_{index}");
        generated_cases.push(expand_case(case, &case_module, &rut, context.as_ref())?);
        case_factories.push(quote!(#case_module::__rut_case()));
    }

    let setup_call = async_block_call(setup);
    let teardown_call = async_block_call(teardown);
    let setup_body = if let Some(context_type) = context.as_ref() {
        quote! {
            let mut context = #rut::__private::ContextInitializer::<#context_type>::new();
            #setup_call
            let context = context.into_inner().expect(
                "suite setup did not initialize its declared context",
            );
            self.context = ::std::option::Option::Some(#rut::TestContext::new(context));
        }
    } else {
        setup_call
    };
    let teardown_body = if let Some(context_type) = context.as_ref() {
        quote! {
            let context = self.context.as_ref()
                .expect("suite context is not initialized")
                .downcast_ref::<#context_type>()
                .expect("suite context does not match its declared type");
            #teardown_call
        }
    } else {
        teardown_call
    };

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #module_ident {
            use super::*;

            #(#helper_items)*
            #(#generated_cases)*

            #[derive(Clone)]
            pub struct #type_ident {
                context: ::std::option::Option<#rut::TestContext>,
                test_cases: ::std::vec::Vec<::std::boxed::Box<dyn #rut::TestCase>>,
            }

            impl #type_ident {
                pub fn new() -> Self {
                    Self {
                        context: ::std::option::Option::None,
                        test_cases: ::std::vec![#(::std::boxed::Box::new(#case_factories)),*],
                    }
                }
            }

            impl ::std::default::Default for #type_ident {
                fn default() -> Self {
                    Self::new()
                }
            }

            #[#rut::__private::async_trait]
            impl #rut::TestSuite for #type_ident {
                async fn setup_suite(&mut self) {
                    #setup_body
                }

                fn context(&self) -> ::std::option::Option<&#rut::TestContext> {
                    self.context.as_ref()
                }

                fn context_mut(&mut self) -> &mut ::std::option::Option<#rut::TestContext> {
                    &mut self.context
                }

                async fn teardown_suite(&mut self) {
                    #teardown_body
                }

                fn name(&self) -> &str {
                    #name
                }

                fn test_cases(&self) -> ::std::vec::Vec<::std::boxed::Box<dyn #rut::TestCase>> {
                    self.test_cases.clone()
                }

                fn test_cases_mut(&mut self) -> &mut ::std::vec::Vec<::std::boxed::Box<dyn #rut::TestCase>> {
                    &mut self.test_cases
                }
            }
        }

        #visibility use #module_ident::#type_ident;
    })
}

fn expand_case(
    case: CaseDecl,
    module_ident: &Ident,
    rut: &proc_macro2::TokenStream,
    context_type: Option<&Type>,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut setup = None;
    let mut teardown = None;
    let mut tests = Vec::new();
    let mut helper_items = Vec::new();
    let mut test_names = HashSet::new();

    for entry in case.entries {
        match entry {
            CaseEntry::Item(item) => helper_items.push(item),
            CaseEntry::Setup(block) => set_once_value(&mut setup, block, "setup")?,
            CaseEntry::Teardown(block) => set_once_value(&mut teardown, block, "teardown")?,
            CaseEntry::Test(test) => {
                if !test_names.insert(test.name.value()) {
                    return Err(Error::new_spanned(test.name, "duplicate test name"));
                }
                tests.push(test);
            }
        }
    }

    if tests.is_empty() {
        return Err(Error::new_spanned(
            case.name,
            "a test case must contain at least one `test(...) { ... }` declaration",
        ));
    }

    let case_name = case.name;
    let setup_call = async_block_call(setup);
    let teardown_call = async_block_call(teardown);
    let context_binding = context_type.map(|context_type| {
        quote! {
            let context = context
                .expect("suite context is not initialized")
                .downcast_ref::<#context_type>()
                .expect("suite context does not match its declared type");
        }
    });
    let context_parameter = if context_type.is_some() {
        quote!(context)
    } else {
        quote!(_context)
    };
    let mut test_impls = Vec::new();
    let mut test_types = Vec::new();

    for (index, test) in tests.into_iter().enumerate() {
        let test_type = format_ident!("__RutGeneratedTest{index}");
        let test_name = test.name;
        let property_keys = test.properties.iter().map(|(key, _)| key);
        let property_values = test.properties.iter().map(|(_, value)| value);
        let test_block = test.block;
        test_types.push(test_type.clone());
        test_impls.push(quote! {
            struct #test_type;

            #[#rut::__private::async_trait]
            impl #rut::Test for #test_type {
                fn name(&self) -> &str {
                    #test_name
                }

                async fn run(
                    &self,
                    #context_parameter: ::std::option::Option<&#rut::TestContext>,
                ) -> #rut::TestResult {
                    #context_binding
                    let mut result: #rut::TestResult = (async #test_block).await;
                    result.name = #test_name.to_owned();
                    #(result.properties.push((#property_keys.to_owned(), #property_values.to_owned()));)*
                    result
                }
            }
        });
    }

    Ok(quote! {
        mod #module_ident {
            use super::*;

            #(#helper_items)*
            #(#test_impls)*

            #[derive(Clone)]
            struct __RutGeneratedCase;

            #[#rut::__private::async_trait]
            impl #rut::TestCase for __RutGeneratedCase {
                fn name(&self) -> &str {
                    #case_name
                }

                async fn setup_case(
                    &mut self,
                    #context_parameter: ::std::option::Option<&#rut::TestContext>,
                ) {
                    #context_binding
                    #setup_call
                }

                async fn teardown_case(
                    &mut self,
                    #context_parameter: ::std::option::Option<&#rut::TestContext>,
                ) {
                    #context_binding
                    #teardown_call
                }

                fn tests(&self) -> ::std::vec::Vec<::std::boxed::Box<dyn #rut::Test>> {
                    ::std::vec![#(::std::boxed::Box::new(#test_types)),*]
                }

                fn clone_box(&self) -> ::std::boxed::Box<dyn #rut::TestCase> {
                    ::std::boxed::Box::new(self.clone())
                }
            }

            pub(super) fn __rut_case() -> impl #rut::TestCase + Clone {
                __RutGeneratedCase
            }
        }
    })
}

fn async_block_call(block: Option<Block>) -> proc_macro2::TokenStream {
    block
        .map(|block| quote!((async #block).await;))
        .unwrap_or_default()
}

fn set_once_value<T>(slot: &mut Option<T>, value: T, name: &str) -> syn::Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            format!("duplicate `{name} async` declaration in test case"),
        ));
    }
    Ok(())
}

fn set_once_at<T>(slot: &mut Option<T>, value: T, ident: &Ident, name: &str) -> syn::Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(
            ident.span(),
            format!("duplicate `{name}` declaration"),
        ));
    }
    Ok(())
}

fn rut_path() -> proc_macro2::TokenStream {
    match crate_name("rut") {
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#ident)
        }
        Ok(FoundCrate::Itself) | Err(_) => quote!(::rut),
    }
}

#[cfg(test)]
mod tests {
    use super::{DslBlock, SuiteDecl, TestArgs};

    const SUITE_BODY: &str = r#"
        test_case(name = "case") {
            test(name = "test") { ::rut::TestResult::passed() }
        }
    "#;

    #[test]
    fn parses_typename_visibility_and_helper_structs() {
        for header in [
            "typename = Suite;",
            "typename = pub Suite;",
            "typename = pub(crate) Suite;",
        ] {
            let declaration = format!(
                r#"{header} name = "suite"; struct First; struct Second {{ value: usize }} {SUITE_BODY}"#,
            );
            let declaration = syn::parse_str::<SuiteDecl>(&declaration).unwrap();
            assert_eq!(declaration.type_ident, "Suite");
            assert_eq!(declaration.helper_items.len(), 2);
        }
    }

    #[test]
    fn rejects_old_or_invalid_typename_declarations() {
        for header in [
            "struct Suite;",
            "typename Suite;",
            "typename = crate::Suite;",
            "typename = Suite<T>;",
            "typename = &Suite;",
        ] {
            let declaration = format!(r#"{header} name = "suite"; {SUITE_BODY}"#);
            assert!(syn::parse_str::<SuiteDecl>(&declaration).is_err());
        }
    }

    #[test]
    fn parses_typed_context_declarations() {
        let declaration = format!(
            r#"typename = Suite; name = "suite"; context = crate::Context<String>; {SUITE_BODY}"#,
        );
        assert!(syn::parse_str::<SuiteDecl>(&declaration).is_ok());
    }

    #[test]
    fn rejects_old_or_malformed_context_declarations() {
        let old = format!(
            r#"typename = Suite; name = "suite"; context {{ Context::default() }} {SUITE_BODY}"#,
        );
        let missing_semicolon =
            format!(r#"typename = Suite; name = "suite"; context = Context {SUITE_BODY}"#,);
        assert!(syn::parse_str::<SuiteDecl>(&old).is_err());
        assert!(syn::parse_str::<SuiteDecl>(&missing_semicolon).is_err());
    }

    #[test]
    fn parses_dsl_blocks_with_or_without_async_keyword() {
        assert!(syn::parse_str::<DslBlock>("{}").is_ok());
        assert!(syn::parse_str::<DslBlock>("async {}").is_ok());
    }

    #[test]
    fn rejects_malformed_dsl_blocks() {
        assert!(syn::parse_str::<DslBlock>("").is_err());
        assert!(syn::parse_str::<DslBlock>("async").is_err());
    }

    #[test]
    fn parses_test_properties_in_declaration_order() {
        let args: TestArgs =
            syn::parse_str(r#"category = "runtime", name = "adds", category = "arithmetic""#)
                .unwrap();

        assert_eq!(args.name.value(), "adds");
        assert_eq!(
            args.properties
                .iter()
                .map(|(key, value)| (key.value(), value.value()))
                .collect::<Vec<_>>(),
            [
                ("category".into(), "runtime".into()),
                ("category".into(), "arithmetic".into())
            ]
        );
    }

    #[test]
    fn requires_exactly_one_name() {
        assert!(syn::parse_str::<TestArgs>(r#"category = "arithmetic""#).is_err());
        assert!(syn::parse_str::<TestArgs>(r#"name = "one", name = "two""#).is_err());
        assert!(syn::parse_str::<TestArgs>(r#"name = """#).is_err());
    }

    #[test]
    fn requires_string_values() {
        assert!(syn::parse_str::<TestArgs>(r#"name = "adds", priority = 1"#).is_err());
    }
}
