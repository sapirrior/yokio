//! Zero-dependency procedural macros for Yokio (`#[yokio::main]` and `#[yokio::test]`).

use proc_macro::{Delimiter, TokenStream, TokenTree};

/// Attributes applied to the root entry point function to drive the Yokio runtime.
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_entry(item, false)
}

/// Attributes applied to test functions to run tests within a Yokio runtime.
#[proc_macro_attribute]
pub fn test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_entry(item, true)
}

fn expand_entry(item: TokenStream, is_test: bool) -> TokenStream {
    let trees: Vec<TokenTree> = item.into_iter().collect();

    // Check for `async` keyword and emit helpful compiler error
    for tree in &trees {
        if let TokenTree::Ident(ident) = tree {
            if ident.to_string() == "async" {
                return r#"compile_error!("Yokio is callback-native and runs on synchronous event loops.\nRemove `async` and use callbacks directly (e.g. `yokio::task`, `yokio::after`, `yokio::watch`).");"#.parse().unwrap();
            }
        }
    }

    let mut i = 0;
    let mut vis = String::new();
    while i < trees.len() {
        if let TokenTree::Ident(ident) = &trees[i] {
            if ident.to_string() == "pub" {
                vis.push_str("pub ");
                i += 1;
                if i < trees.len() {
                    if let TokenTree::Group(group) = &trees[i] {
                        if group.delimiter() == Delimiter::Parenthesis {
                            vis.push_str(&format!("({}) ", group.stream()));
                            i += 1;
                        }
                    }
                }
                continue;
            }
            if ident.to_string() == "fn" {
                i += 1;
                break;
            }
        }
        i += 1;
    }

    if i >= trees.len() {
        return r#"compile_error!("`#[yokio::main]` must be placed on a function definition.");"#
            .parse()
            .unwrap();
    }

    let fn_name = match &trees[i] {
        TokenTree::Ident(ident) => ident.to_string(),
        _ => return r#"compile_error!("expected function name");"#.parse().unwrap(),
    };
    i += 1;

    if i >= trees.len() {
        return r#"compile_error!("expected function parameters");"#
            .parse()
            .unwrap();
    }
    match &trees[i] {
        TokenTree::Group(group) if group.delimiter() == Delimiter::Parenthesis => {
            if !group.stream().is_empty() {
                return r#"compile_error!("entry function must have zero arguments");"#
                    .parse()
                    .unwrap();
            }
        }
        _ => {
            return r#"compile_error!("expected function parameters in parentheses");"#
                .parse()
                .unwrap()
        }
    }
    i += 1;

    let mut return_type = String::new();
    let mut body_group = None;

    while i < trees.len() {
        match &trees[i] {
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => {
                body_group = Some(group.stream());
                break;
            }
            tt => {
                return_type.push_str(&tt.to_string());
                return_type.push(' ');
            }
        }
        i += 1;
    }

    let Some(body) = body_group else {
        return r#"compile_error!("expected function body block `{ ... }`");"#
            .parse()
            .unwrap();
    };

    let ret_decl = if return_type.trim().is_empty() {
        String::new()
    } else {
        return_type.trim().to_string()
    };

    let test_attr = if is_test { "#[test]\n" } else { "" };

    let expanded = format!(
        r#"{test_attr}{vis}fn {fn_name}() {ret_decl} {{
    let __yokio_rt = ::yokio::Runtime::new().expect("failed to create Yokio runtime");
    __yokio_rt.run_with(move || {{
        {body}
    }}).expect("Yokio runtime execution failed")
}}"#
    );

    expanded.parse().unwrap()
}
