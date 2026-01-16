use proc_macro::{Delimiter, Group, TokenStream, TokenTree};

fn compile_error(msg: &str) -> TokenStream {
    format!("compile_error!({msg:?});").parse().unwrap()
}

fn is_ident(tt: &TokenTree, s: &str) -> bool {
    matches!(tt, TokenTree::Ident(id) if id.to_string() == s)
}

#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return compile_error("eventloop_async_research::main does not accept arguments yet");
    }

    let mut it = item.into_iter().peekable();

    let mut leading_attrs: Vec<TokenTree> = Vec::new();
    loop {
        let Some(tt) = it.peek() else { break };
        if matches!(tt, TokenTree::Punct(p) if p.as_char() == '#') {
            leading_attrs.push(it.next().unwrap());
            let Some(group) = it.next() else {
                return compile_error("expected #[..] after #");
            };
            leading_attrs.push(group);
            continue;
        }
        break;
    }

    if matches!(it.peek(), Some(TokenTree::Ident(id)) if id.to_string() == "pub") {
        let _ = it.next();
    }

    let Some(tt) = it.next() else {
        return compile_error("expected `async fn main`");
    };
    if !is_ident(&tt, "async") {
        return compile_error("expected `async fn main` (missing `async`)");
    }

    let Some(tt) = it.next() else {
        return compile_error("expected `fn`");
    };
    if !is_ident(&tt, "fn") {
        return compile_error("expected `fn`");
    }

    let Some(tt) = it.next() else {
        return compile_error("expected `main`");
    };
    if !is_ident(&tt, "main") {
        return compile_error("only `async fn main` is supported");
    }

    let Some(tt) = it.next() else {
        return compile_error("expected `()`");
    };
    match tt {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis => {
            if !g.stream().is_empty() {
                return compile_error("`main` arguments are not supported");
            }
        }
        _ => return compile_error("expected `()` after `main`"),
    }

    let mut return_toks: Vec<TokenTree> = Vec::new();
    let mut body: Option<Group> = None;

    let has_arrow = {
        let first_is_dash = matches!(it.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '-');
        let second_is_gt = {
            let mut look = it.clone();
            let _ = look.next();
            matches!(look.next(), Some(TokenTree::Punct(p)) if p.as_char() == '>')
        };
        first_is_dash && second_is_gt
    };

    if has_arrow {
        let _ = it.next();
        let _ = it.next();

        while let Some(tt) = it.next() {
            match tt {
                TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                    body = Some(g);
                    break;
                }
                other => return_toks.push(other),
            }
        }
    }

    if body.is_none() {
        while let Some(tt) = it.next() {
            match tt {
                TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                    body = Some(g);
                    break;
                }
                TokenTree::Punct(p) if p.as_char() == ';' => {}
                _ => return compile_error("unexpected tokens in `main` signature"),
            }
        }
    }

    let Some(body) = body else {
        return compile_error("expected `{ ... }` body for `main`");
    };

    let ret_str = {
        let ts: TokenStream = return_toks.iter().cloned().collect();
        ts.to_string()
    };
    let ret_norm: String = ret_str.chars().filter(|c| !c.is_whitespace()).collect();

    let crate_path = "::eventloop_async_research";
    let body_str = body.stream().to_string();
    let attrs_str = {
        let ts: TokenStream = leading_attrs.iter().cloned().collect();
        ts.to_string()
    };

    if return_toks.is_empty() || ret_norm == "()" {
        let out = format!(
            "{attrs_str}\nfn main() {{ {crate_path}::run({crate_path}::backend_from_args(), async move {{ {body_str} }}).unwrap(); }}",
        );
        return out.parse().unwrap();
    }

    for prefix in [
        "std::io::Result<",
        "::std::io::Result<",
        "io::Result<",
        "::io::Result<",
    ] {
        if ret_norm.starts_with(prefix) && ret_norm.ends_with('>') {
            let out = format!(
                "{attrs_str}\nfn main() -> {ret_str} {{ {crate_path}::run({crate_path}::backend_from_args(), async move {{ {body_str} }})? }}",
            );
            return out.parse().unwrap();
        }
    }

    compile_error("unsupported return type for #[eventloop_async_research::main]; use () or std::io::Result<T>")
}
