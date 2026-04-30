//! Derive macro for `structkey::Codec`.
//!
//! Generates a `Codec` impl for a struct with named fields by
//! delegating to each field's own `Codec` impl, in declaration order.
//! `encode_key` threads the segment limit `n` through fields,
//! decrementing by each field's `segment_count`; `segment_count` returns
//! the sum.
//!
//! # Crate path resolution
//!
//! At expansion time the macro looks up the consumer's `Cargo.toml` (via
//! `proc-macro-crate`) to find the actual extern name for `structkey`.
//! This handles Cargo renames such as `mykey = { package = "structkey" }`
//! transparently. Consumers must take a direct dep on `structkey`;
//! reaching it transitively through another crate's re-export is not
//! supported.
//!
//! # Field attributes
//!
//! - `#[codec(raw)]` — route the field through `Raw`'s `Codec`
//!   impl, which uses `push_raw` / `next_raw` and skips percent-escaping.
//!   Only valid on `String` fields; the value must not contain the
//!   segment separator `/`.

use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro_crate::crate_name;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Data;
use syn::DataStruct;
use syn::DeriveInput;
use syn::Field;
use syn::Fields;
use syn::Ident;
use syn::parse_macro_input;

#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(named),
            ..
        }) => &named.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "#[derive(Codec)] supports only structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let field_idents: Vec<&Ident> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();

    let raw_flags: Vec<bool> = match fields.iter().map(check_raw).collect() {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let kv = structkey_root();
    let n_fields = fields.len();

    // For each field: encode it with the current `n`. For all but the
    // last field, follow with `let n = n.saturating_sub(field.segment_count())`
    // so the next field sees a smaller allowance. The trailing field
    // omits the `let n =`, since nothing reads `n` afterward.
    let encode_stmts: Vec<TokenStream2> = fields
        .iter()
        .zip(raw_flags.iter())
        .enumerate()
        .map(|(i, (f, &raw))| {
            let name = f.ident.as_ref().unwrap();
            let receiver = if raw {
                quote! { #kv::Raw::from_ref(&self.#name) }
            } else {
                quote! { &self.#name }
            };
            let encode = quote! {
                let b = #kv::Codec::encode_key(#receiver, b, n);
            };
            if i + 1 < n_fields {
                quote! {
                    #encode
                    let n = n.saturating_sub(#kv::Codec::segment_count(#receiver));
                }
            } else {
                encode
            }
        })
        .collect();

    let decode_lets: Vec<TokenStream2> = fields
        .iter()
        .zip(raw_flags.iter())
        .map(|(f, &raw)| {
            let name = f.ident.as_ref().unwrap();
            if raw {
                // Decode as `Raw`, then unwrap to the underlying `String`.
                quote! { let #name = <#kv::Raw as #kv::Codec>::decode_key(p)?.into_inner(); }
            } else {
                quote! { let #name = #kv::Codec::decode_key(p)?; }
            }
        })
        .collect();

    let segment_count_expr = if fields.is_empty() {
        quote! { 0 }
    } else {
        let parts: Vec<TokenStream2> = fields
            .iter()
            .zip(raw_flags.iter())
            .map(|(f, &raw)| {
                let name = f.ident.as_ref().unwrap();
                if raw {
                    quote! { #kv::Codec::segment_count(#kv::Raw::from_ref(&self.#name)) }
                } else {
                    quote! { #kv::Codec::segment_count(&self.#name) }
                }
            })
            .collect();
        quote! { #(#parts)+* }
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics #kv::Codec
            for #name #ty_generics #where_clause
        {
            #[allow(unused_variables, clippy::let_and_return)]
            fn encode_key(&self, b: #kv::Builder, n: usize) -> #kv::Builder {
                #(#encode_stmts)*
                b
            }

            #[allow(unused_variables)]
            fn decode_key(
                p: &mut #kv::Parser,
            ) -> ::std::result::Result<Self, #kv::Error>
            where Self: Sized
            {
                #(#decode_lets)*
                ::std::result::Result::Ok(Self { #(#field_idents),* })
            }

            fn segment_count(&self) -> usize {
                #segment_count_expr
            }
        }
    };

    expanded.into()
}

/// Resolve the crate path the macro should emit for `structkey` types.
///
/// - **Direct dep, possibly renamed.** `proc-macro-crate` returns the
///   consumer's extern name (e.g. `mykey` for
///   `mykey = { package = "structkey" }`). Macro emits `::mykey::*`.
/// - **Self.** When invoked inside `structkey` itself (in-crate tests
///   and doctests), emits `::structkey::*`. The crate aliases itself
///   with `extern crate self as structkey;` at the lib root, so this
///   path resolves both inside the crate and in doctests (which link
///   `structkey` as an external dep).
/// - **No direct dep.** Falls back to `::structkey::*`. If that name
///   resolves nowhere, the compiler's "crate not found" error points
///   at a recognisable path.
fn structkey_root() -> TokenStream2 {
    match crate_name("structkey") {
        Ok(FoundCrate::Itself) => quote! { ::structkey },
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote! { ::#ident }
        }
        Err(_) => quote! { ::structkey },
    }
}

/// Returns `Ok(true)` if the field carries `#[codec(raw)]`.
///
/// Returns `Err` for any unknown sub-option, so typos like
/// `#[codec(rwa)]` fail loudly instead of silently behaving as
/// "not raw".
fn check_raw(field: &Field) -> syn::Result<bool> {
    let mut found = false;
    for attr in &field.attrs {
        if !attr.path().is_ident("codec") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("raw") {
                found = true;
                Ok(())
            } else {
                Err(meta.error("unknown #[codec] option; expected `raw`"))
            }
        })?;
    }
    Ok(found)
}
