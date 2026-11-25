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

    // Verify parameters: find parenthesis group
    let mut param_found = false;
    for tree in &trees {
        if let TokenTree::Group(group) = tree {
            if group.delimiter() == Delimiter::Parenthesis {
                if !group.stream().is_empty() {
                    return r#"compile_error!("entry function must have zero arguments");"#
                        .parse()
                        .unwrap();
                }
                param_found = true;
                break;
            }
        }
    }

    if !param_found {
        return r#"compile_error!("expected function definition with parameter list `()`");"#
            .parse()
            .unwrap();
    }

    // Find the body block `{ ... }`
    let mut brace_idx = None;
    for (idx, tree) in trees.iter().enumerate() {
        if let TokenTree::Group(group) = tree {
            if group.delimiter() == Delimiter::Brace {
                brace_idx = Some(idx);
                break;
            }
        }
    }

    let Some(idx) = brace_idx else {
        return r#"compile_error!("expected function body block `{ ... }`");"#
            .parse()
            .unwrap();
    };

    let sig: TokenStream = trees[..idx].iter().cloned().collect();
    let body = match &trees[idx] {
        TokenTree::Group(group) => group.stream(),
        _ => unreachable!(),
    };

    let test_attr = if is_test { "#[test]\n" } else { "" };

    let expanded = format!(
        r#"{test_attr} {sig} {{
    let __yokio_rt = ::yokio::Runtime::new().expect("failed to create Yokio runtime");
    __yokio_rt.run_with(move || {{
        {body}
    }}).expect("Yokio runtime execution failed")
}}"#
    );

    expanded.parse().unwrap()
}
