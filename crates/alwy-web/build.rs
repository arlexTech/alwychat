//! Generates the web dispatch for every Tauri command that only passes its arguments to the
//! core (`app/src-tauri/src/lib.rs`), so the browser gets new commands without copying them.
//! Commands that need the app shell (channels, windows, Android…) are written by hand in
//! `src/lib.rs` and skipped here.

use std::{env, fs, path::Path};

use quote::{ToTokens, quote};
use syn::{FnArg, Item, Pat, ReturnType, Type};

const TAURI_COMMANDS: &str = "../../app/src-tauri/src/lib.rs";

/// Parameter types that only exist in the Tauri shell.
/// (`Channel` isn't one: the web crate has its own, with the same `send`.)
const SHELL_TYPES: [&str; 5] = ["AppHandle", "Window", "WebviewWindow", "Request", "Webview"];

fn main() {
    println!("cargo:rerun-if-changed={TAURI_COMMANDS}");
    let source = fs::read_to_string(TAURI_COMMANDS).expect("Tauri commands");
    let file = syn::parse_file(&source).expect("Tauri lib.rs parses");
    let handwritten = handwritten();

    // The shell's imports of core types, which the copied commands use.
    let imports: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Use(u) if u.to_token_stream().to_string().starts_with("use alwy_core") => {
                Some(u.clone())
            }
            _ => None,
        })
        .collect();
    // Plain helper functions of the shell (no Tauri inside), copied when a command uses one.
    let helpers: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f)
                if !f
                    .attrs
                    .iter()
                    .any(|a| a.path().to_token_stream().to_string().contains("tauri"))
                    && !f.to_token_stream().to_string().contains("tauri")
                    && !f.to_token_stream().to_string().contains("cfg") =>
            {
                Some(f.clone())
            }
            _ => None,
        })
        .collect();
    let mut used_helpers = Vec::new();
    let mut arms = Vec::new();
    let mut names = Vec::new();
    for item in &file.items {
        let Item::Fn(f) = item else { continue };
        if !f
            .attrs
            .iter()
            .any(|a| a.path().to_token_stream().to_string().replace(' ', "") == "tauri::command")
        {
            continue;
        }
        let name = f.sig.ident.to_string();
        if handwritten.contains(&name) {
            continue;
        }
        let body = f.block.to_token_stream().to_string();
        // Platform branches and app-shell calls need a hand-written browser version.
        if body.contains("cfg") || body.contains("app .") || body.contains("tauri ::") {
            continue;
        }
        let mut lets = Vec::new();
        let mut shell = false;
        for input in &f.sig.inputs {
            let FnArg::Typed(arg) = input else { continue };
            let Pat::Ident(ident) = &*arg.pat else {
                shell = true;
                continue;
            };
            let ty = arg.ty.to_token_stream().to_string();
            if ty.starts_with("State") {
                if !ty.contains("AppCore") {
                    shell = true;
                }
                continue;
            }
            if SHELL_TYPES.iter().any(|t| ty.contains(t)) {
                shell = true;
                continue;
            }
            let var = &ident.ident;
            let key = camel(&var.to_string());
            let ty: &Type = &arg.ty;
            if ty.to_token_stream().to_string().starts_with("Channel") {
                lets.push(quote! { let #var: #ty = args.channel(#key)?; });
            } else {
                lets.push(quote! { let #var: #ty = args.get(#key)?; });
            }
        }
        if shell {
            continue;
        }
        let block = &f.block;
        for helper in &helpers {
            let called = format!("{} (", helper.sig.ident);
            if body.contains(&called)
                && !used_helpers
                    .iter()
                    .any(|h: &syn::ItemFn| h.sig.ident == helper.sig.ident)
            {
                used_helpers.push(helper.clone());
            }
        }
        let fallible = match &f.sig.output {
            ReturnType::Default => false,
            ReturnType::Type(_, ty) => ty.to_token_stream().to_string().contains("Result"),
        };
        let call = if f.sig.asyncness.is_some() {
            quote! { async move #block.await }
        } else {
            quote! { #block }
        };
        // Typed as the command declares it (some bodies leave the error type to inference).
        let output = match &f.sig.output {
            ReturnType::Default => quote! { () },
            ReturnType::Type(_, ty) => quote! { #ty },
        };
        let reply = if fallible {
            quote! { let value: #output = #call; fallible(value) }
        } else {
            quote! { let value: #output = #call; infallible(value) }
        };
        arms.push(quote! {
            #name => {
                let reply: Reply = async move { #(#lets)* #reply }.await;
                reply
            }
        });
        names.push(name);
    }

    let code = quote! {
        #[allow(unused_imports)]
        #(#imports)*

        #(#used_helpers)*

        /// Commands that pass straight to the core (generated from the Tauri shell).
        pub(crate) const GENERATED: &[&str] = &[#(#names),*];

        #[allow(unused_variables, clippy::let_unit_value, clippy::unused_async)]
        pub(crate) async fn generated(core: &'static Core, cmd: &str, args: &Args) -> Option<Reply> {
            Some(match cmd {
                #(#arms)*
                _ => return None,
            })
        }
    };
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("generated.rs");
    fs::write(out, code.to_string()).unwrap();
}

/// Commands `src/lib.rs` handles itself: the names in its `HANDWRITTEN` list.
fn handwritten() -> Vec<String> {
    println!("cargo:rerun-if-changed=src/lib.rs");
    let source = fs::read_to_string("src/lib.rs").unwrap_or_default();
    let Some(start) = source.find("const HANDWRITTEN") else {
        return Vec::new();
    };
    let list = &source[start..];
    let list = &list[list.find("= &[").unwrap() + 4..];
    let list = &list[..list.find(']').unwrap()];
    list.split(',')
        .map(|s| s.trim().trim_matches('"').to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}
