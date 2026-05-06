//! Derive macros for `structkey::Codec` and `structkey::StructKey`.
//!
//! Generates a `Codec` impl for a struct with named fields, or for an
//! enum, by delegating to each field's own `Codec` impl in declaration
//! order. `segment_count` returns the sum.
//!
//! For enums, each variant is prefixed with a discriminant segment whose
//! string is the `snake_case` form of the variant identifier (e.g.
//! `Database` -> `database`, `TwoWords` -> `two_words`, `UDF` -> `udf`).
//! The variant's fields follow the discriminant, encoded as they would
//! be in a struct. Effective discriminants, including values supplied by
//! `#[codec(rename = "...")]`, must be unique within the enum.
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
//! # Phantom data
//!
//! Fields whose type's last path segment is `PhantomData` are silently
//! skipped: not encoded, not decoded, and contribute nothing to
//! `segment_count`. This lets the derive cover marker-typed structs
//! such as `Foo<R> { id: u64, _p: PhantomData<R> }` without forcing
//! the marker `R` to implement `Codec`.
//!
//! The detection is by name on the last path segment, so
//! `PhantomData<R>`, `std::marker::PhantomData<R>`, and
//! `core::marker::PhantomData<R>` are all recognised. A user-defined
//! type called `PhantomData` would be a false positive; rename it or
//! hand-write the impl.
//!
//! # Field attributes
//!
//! - `#[codec(raw)]` — route the field through `Raw`'s `Codec`
//!   impl, which uses `push_raw` / `next_raw` and skips percent-escaping.
//!   Only valid on `String` fields; the value must not contain the
//!   segment separator `/`. Works on struct fields and on enum variant
//!   fields alike.
//!
//! # Variant attributes
//!
//! - `#[codec(rename = "...")]` — override the discriminant text for an
//!   enum variant. Use this when the `snake_case` default doesn't match
//!   an existing wire format, or when you want a separator other than
//!   `_` (e.g. `two-words` for kebab-case). Value must be non-empty and
//!   must not contain `/`, and the resulting discriminant must be unique
//!   within the enum.

use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro_crate::crate_name;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Data;
use syn::DataEnum;
use syn::DataStruct;
use syn::DeriveInput;
use syn::Field;
use syn::Fields;
use syn::Ident;
use syn::Variant;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// Derive `StructKey` (and the implied `Codec`) for a type.
///
/// Emits both impls in one go, so users don't need to also write
/// `#[derive(Codec)]` — `#[derive(StructKey)]` covers the trait it
/// requires. Field-level `#[codec(raw)]` and variant-level
/// `#[codec(rename = "...")]` attributes are still recognised because
/// the derive registers the `codec` namespace too.
///
/// The container attribute `#[structkey(prefix = "...")]` supplies the
/// trait's required `PREFIX` constant. The value must be non-empty and
/// must not contain `/` (segment separator); both are rejected at
/// compile time.
///
/// ```ignore
/// #[derive(Debug, PartialEq, Eq, StructKey)]
/// #[structkey(prefix = "session")]
/// struct UserSession {
///     user_id: u64,
///     session: String,
/// }
/// ```
///
/// Combining `#[derive(Codec, StructKey)]` is an error — both derives
/// would emit `impl Codec`, producing a duplicate-impl compile error.
/// Use `#[derive(Codec)]` alone for types that are *parts* of a key
/// (enum variants embedded in larger keys, helper structs without a
/// prefix); use `#[derive(StructKey)]` alone for top-level keys.
#[proc_macro_derive(StructKey, attributes(structkey, codec))]
pub fn derive_struct_key(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_struct_key_inner(&input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn derive_struct_key_inner(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let prefix = parse_struct_key_prefix(input)?;
    let codec_impl = build_codec_impl(input)?;

    let kv = structkey_root();
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let struct_key_impl = quote! {
        #[automatically_derived]
        impl #impl_generics #kv::StructKey for #name #ty_generics #where_clause {
            const PREFIX: &'static str = #prefix;
        }
    };

    Ok(quote! {
        #codec_impl
        #struct_key_impl
    })
}

fn parse_struct_key_prefix(input: &DeriveInput) -> syn::Result<String> {
    let mut prefix: Option<String> = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("structkey") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                let value = lit.value();
                if value.is_empty() {
                    return Err(meta.error("#[structkey(prefix = \"...\")] must not be empty"));
                }
                if value.contains('/') {
                    return Err(meta.error(
                        "#[structkey(prefix = \"...\")] must not contain '/' (segment separator)",
                    ));
                }
                prefix = Some(value);
                Ok(())
            } else {
                Err(meta.error("unknown #[structkey] option; expected `prefix`"))
            }
        })?;
    }
    prefix.ok_or_else(|| {
        syn::Error::new_spanned(
            &input.ident,
            "#[derive(StructKey)] requires `#[structkey(prefix = \"...\")]`",
        )
    })
}

#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    build_codec_impl(&input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Shared between `#[derive(Codec)]` and `#[derive(StructKey)]` (since
/// the latter implies the former). Dispatches on the input's data
/// shape and returns the `impl Codec` token stream.
fn build_codec_impl(input: &DeriveInput) -> syn::Result<TokenStream2> {
    match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(named),
            ..
        }) => derive_struct(input, &named.named),
        Data::Enum(data) => derive_enum(input, data),
        _ => Err(syn::Error::new_spanned(
            &input.ident,
            "#[derive(Codec)] supports structs with named fields and enums",
        )),
    }
}

fn derive_struct(
    input: &DeriveInput,
    fields: &Punctuated<Field, Comma>,
) -> syn::Result<TokenStream2> {
    let raw_flags: Vec<bool> = fields.iter().map(check_raw).collect::<syn::Result<_>>()?;
    let phantom_flags: Vec<bool> = fields.iter().map(|f| is_phantom_data(&f.ty)).collect();

    let kv = structkey_root();

    // Each non-phantom field encodes itself onto the running builder.
    // PhantomData fields carry no data and are silently skipped, which
    // is what lets the derive work for marker-typed structs like
    // `Foo<R> { id: u64, _p: PhantomData<R> }` without forcing
    // `R: Codec`.
    let encode_stmts: Vec<TokenStream2> = fields
        .iter()
        .zip(raw_flags.iter())
        .zip(phantom_flags.iter())
        .filter_map(|((f, &raw), &phantom)| {
            if phantom {
                return None;
            }
            let name = f.ident.as_ref().unwrap();
            let receiver = if raw {
                quote! { #kv::Raw::from_ref(&self.#name) }
            } else {
                quote! { &self.#name }
            };
            Some(quote! {
                let b = #kv::Codec::encode_key(#receiver, b);
            })
        })
        .collect();

    // Decode runs once per non-phantom field; phantom fields fill in
    // through `::core::marker::PhantomData` directly in the struct
    // construction below, where Self's declared field type drives the
    // missing type parameter via inference.
    let decode_lets: Vec<TokenStream2> = fields
        .iter()
        .zip(raw_flags.iter())
        .zip(phantom_flags.iter())
        .filter_map(|((f, &raw), &phantom)| {
            if phantom {
                return None;
            }
            let name = f.ident.as_ref().unwrap();
            Some(if raw {
                quote! { let #name = <#kv::Raw as #kv::Codec>::decode_key(p)?.into_inner(); }
            } else {
                quote! { let #name = #kv::Codec::decode_key(p)?; }
            })
        })
        .collect();

    let field_constructors: Vec<TokenStream2> = fields
        .iter()
        .zip(phantom_flags.iter())
        .map(|(f, &phantom)| {
            let name = f.ident.as_ref().unwrap();
            if phantom {
                quote! { #name: ::core::marker::PhantomData }
            } else {
                quote! { #name }
            }
        })
        .collect();

    let segment_count_parts: Vec<TokenStream2> = fields
        .iter()
        .zip(raw_flags.iter())
        .zip(phantom_flags.iter())
        .filter_map(|((f, &raw), &phantom)| {
            if phantom {
                return None;
            }
            let name = f.ident.as_ref().unwrap();
            Some(if raw {
                quote! { #kv::Codec::segment_count(#kv::Raw::from_ref(&self.#name)) }
            } else {
                quote! { #kv::Codec::segment_count(&self.#name) }
            })
        })
        .collect();

    let segment_count_expr = if segment_count_parts.is_empty() {
        quote! { 0 }
    } else {
        quote! { #(#segment_count_parts)+* }
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #kv::Codec
            for #name #ty_generics #where_clause
        {
            #[allow(unused_variables, clippy::let_and_return)]
            fn encode_key(&self, b: #kv::Builder) -> #kv::Builder {
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
                ::std::result::Result::Ok(Self { #(#field_constructors),* })
            }

            fn segment_count(&self) -> usize {
                #segment_count_expr
            }
        }
    })
}

fn derive_enum(input: &DeriveInput, data: &DataEnum) -> syn::Result<TokenStream2> {
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "#[derive(Codec)] does not support empty enums (no variants to discriminate)",
        ));
    }

    let kv = structkey_root();
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // One discriminant string per variant, used as the leading raw
    // segment in both encode and decode. Default: snake_case of the
    // ident (`Database` -> `database`, `TwoWords` -> `two_words`,
    // `UDF` -> `udf`, `RowAccessPolicy` -> `row_access_policy`). Set
    // `#[codec(rename = "...")]` on a variant to choose a different
    // tag.
    let tags: Vec<String> = data
        .variants
        .iter()
        .map(|v| {
            check_variant_rename(v).map(|r| r.unwrap_or_else(|| snake_case(&v.ident.to_string())))
        })
        .collect::<syn::Result<_>>()?;
    check_unique_variant_tags(data, &tags)?;

    let mut encode_arms: Vec<TokenStream2> = Vec::with_capacity(data.variants.len());
    let mut decode_arms: Vec<TokenStream2> = Vec::with_capacity(data.variants.len());
    let mut count_arms: Vec<TokenStream2> = Vec::with_capacity(data.variants.len());

    for (variant, tag) in data.variants.iter().zip(tags.iter()) {
        let parts = build_variant_parts(variant, tag, &kv)?;
        encode_arms.push(parts.encode);
        decode_arms.push(parts.decode);
        count_arms.push(parts.count);
    }

    // Catch-all for an unknown discriminant. `__tag` is a `&str` borrow
    // of the parser's segment buffer; the named arms above don't refer
    // to it, so calling `p.next_*` inside them is fine. It survives
    // into this arm because it's the only place we still need it.
    let expect_msg = tags.join("|");

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #kv::Codec
            for #name #ty_generics #where_clause
        {
            fn encode_key(&self, b: #kv::Builder) -> #kv::Builder {
                match self {
                    #(#encode_arms)*
                }
            }

            fn decode_key(
                p: &mut #kv::Parser,
            ) -> ::std::result::Result<Self, #kv::Error>
            where Self: Sized
            {
                let __tag = p.next_raw()?;
                match __tag {
                    #(#decode_arms)*
                    _ => ::std::result::Result::Err(#kv::Error::InvalidSegment {
                        i: p.index(),
                        expect: #expect_msg.to_string(),
                        got: __tag.to_string(),
                    }),
                }
            }

            fn segment_count(&self) -> usize {
                match self {
                    #(#count_arms)*
                }
            }
        }
    })
}

fn check_unique_variant_tags(data: &DataEnum, tags: &[String]) -> syn::Result<()> {
    let mut seen: Vec<(&str, &Ident)> = Vec::new();

    for (variant, tag) in data.variants.iter().zip(tags.iter()) {
        if let Some((_, first_ident)) = seen.iter().find(|(seen_tag, _)| *seen_tag == tag) {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                format!(
                    "duplicate #[codec] enum discriminant `{}`; first used by variant `{}`",
                    tag, first_ident
                ),
            ));
        }

        seen.push((tag.as_str(), &variant.ident));
    }

    Ok(())
}

struct VariantParts {
    encode: TokenStream2,
    decode: TokenStream2,
    count: TokenStream2,
}

fn build_variant_parts(
    variant: &Variant,
    tag: &str,
    kv: &TokenStream2,
) -> syn::Result<VariantParts> {
    let v_ident = &variant.ident;

    match &variant.fields {
        Fields::Named(named) => {
            let raw_flags: Vec<bool> = named
                .named
                .iter()
                .map(check_raw)
                .collect::<syn::Result<_>>()?;
            let phantom_flags: Vec<bool> =
                named.named.iter().map(|f| is_phantom_data(&f.ty)).collect();
            let field_idents: Vec<&Ident> = named
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();

            // Pattern bindings: real synthetic name for active fields,
            // `_` for phantom positions (can't omit -- enums require
            // every field to appear or use `..`, and a real binding for
            // a never-encoded field would draw `unused_variables`).
            let bindings: Vec<TokenStream2> = field_idents
                .iter()
                .zip(phantom_flags.iter())
                .enumerate()
                .map(|(i, (field, &phantom))| {
                    if phantom {
                        quote! { #field: _ }
                    } else {
                        let s = synth_name(i);
                        quote! { #field: #s }
                    }
                })
                .collect();

            let active = active_field_iter(&raw_flags, &phantom_flags);

            let encode_stmts = active.clone().map(|(i, raw)| {
                let s = synth_name(i);
                let receiver = if raw {
                    quote! { #kv::Raw::from_ref(#s) }
                } else {
                    quote! { #s }
                };
                quote! { let b = #kv::Codec::encode_key(#receiver, b); }
            });

            let encode = quote! {
                Self::#v_ident { #(#bindings),* } => {
                    let b = b.push_raw(#tag);
                    #(#encode_stmts)*
                    b
                }
            };

            let decode_assigns = field_idents
                .iter()
                .zip(raw_flags.iter())
                .zip(phantom_flags.iter())
                .map(|((field, &raw), &phantom)| {
                    if phantom {
                        quote! { #field: ::core::marker::PhantomData }
                    } else if raw {
                        quote! { #field: <#kv::Raw as #kv::Codec>::decode_key(p)?.into_inner() }
                    } else {
                        quote! { #field: #kv::Codec::decode_key(p)? }
                    }
                });

            let decode = quote! {
                #tag => ::std::result::Result::Ok(Self::#v_ident {
                    #(#decode_assigns),*
                }),
            };

            let count_terms = active.clone().map(|(i, raw)| {
                let s = synth_name(i);
                if raw {
                    quote! { #kv::Codec::segment_count(#kv::Raw::from_ref(#s)) }
                } else {
                    quote! { #kv::Codec::segment_count(#s) }
                }
            });

            let count = if active.clone().count() == 0 {
                quote! { Self::#v_ident { .. } => 1, }
            } else {
                quote! {
                    Self::#v_ident { #(#bindings),* } => 1 #(+ #count_terms)*,
                }
            };

            Ok(VariantParts {
                encode,
                decode,
                count,
            })
        }

        Fields::Unnamed(unnamed) => {
            let raw_flags: Vec<bool> = unnamed
                .unnamed
                .iter()
                .map(check_raw)
                .collect::<syn::Result<_>>()?;
            let phantom_flags: Vec<bool> = unnamed
                .unnamed
                .iter()
                .map(|f| is_phantom_data(&f.ty))
                .collect();

            // Same pattern story as named: synthetic for active, `_`
            // for phantom positions.
            let bindings: Vec<TokenStream2> = phantom_flags
                .iter()
                .enumerate()
                .map(|(i, &phantom)| {
                    if phantom {
                        quote! { _ }
                    } else {
                        let s = synth_name(i);
                        quote! { #s }
                    }
                })
                .collect();

            let active = active_field_iter(&raw_flags, &phantom_flags);

            let encode_stmts = active.clone().map(|(i, raw)| {
                let s = synth_name(i);
                let receiver = if raw {
                    quote! { #kv::Raw::from_ref(#s) }
                } else {
                    quote! { #s }
                };
                quote! { let b = #kv::Codec::encode_key(#receiver, b); }
            });

            let encode = quote! {
                Self::#v_ident(#(#bindings),*) => {
                    let b = b.push_raw(#tag);
                    #(#encode_stmts)*
                    b
                }
            };

            let decode_calls =
                raw_flags
                    .iter()
                    .zip(phantom_flags.iter())
                    .map(|(&raw, &phantom)| {
                        if phantom {
                            quote! { ::core::marker::PhantomData }
                        } else if raw {
                            quote! { <#kv::Raw as #kv::Codec>::decode_key(p)?.into_inner() }
                        } else {
                            quote! { #kv::Codec::decode_key(p)? }
                        }
                    });

            let decode = quote! {
                #tag => ::std::result::Result::Ok(Self::#v_ident(#(#decode_calls),*)),
            };

            let count_terms = active.clone().map(|(i, raw)| {
                let s = synth_name(i);
                if raw {
                    quote! { #kv::Codec::segment_count(#kv::Raw::from_ref(#s)) }
                } else {
                    quote! { #kv::Codec::segment_count(#s) }
                }
            });

            let count = if active.clone().count() == 0 {
                quote! { Self::#v_ident(..) => 1, }
            } else {
                quote! {
                    Self::#v_ident(#(#bindings),*) => 1 #(+ #count_terms)*,
                }
            };

            Ok(VariantParts {
                encode,
                decode,
                count,
            })
        }

        Fields::Unit => Ok(VariantParts {
            encode: quote! { Self::#v_ident => b.push_raw(#tag), },
            decode: quote! { #tag => ::std::result::Result::Ok(Self::#v_ident), },
            count: quote! { Self::#v_ident => 1, },
        }),
    }
}

fn synth_name(i: usize) -> Ident {
    Ident::new(&format!("__f{}", i), Span::call_site())
}

/// Walks `(raw_flags, phantom_flags)` and yields `(position, raw)` for
/// every non-phantom field, preserving original positions so the
/// caller can reconstruct synthetic names with `synth_name(i)`.
fn active_field_iter<'a>(
    raw_flags: &'a [bool],
    phantom_flags: &'a [bool],
) -> impl Iterator<Item = (usize, bool)> + Clone + 'a {
    raw_flags
        .iter()
        .zip(phantom_flags.iter())
        .enumerate()
        .filter_map(|(i, (&raw, &phantom))| if phantom { None } else { Some((i, raw)) })
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

/// Recognise `PhantomData<…>` field types so the derive can skip them.
///
/// Match-by-name on the last path segment, so `PhantomData<R>`,
/// `std::marker::PhantomData<R>`, `core::marker::PhantomData<R>`, and
/// `marker::PhantomData<R>` are all detected. A user-defined type that
/// happens to be named `PhantomData` would be a false positive, but
/// that's very unlikely; documented at the top-level `# Phantom data`
/// section.
fn is_phantom_data(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    tp.path
        .segments
        .last()
        .is_some_and(|s| s.ident == "PhantomData")
}

/// Convert a Rust identifier to `snake_case`.
///
/// Inserts `_` before each uppercase letter that starts a new word --
/// either after a lowercase letter (`TwoWords` -> `two_words`) or at the
/// boundary between an acronym and a following word (`XMLParser` ->
/// `xml_parser`, `IPAddress` -> `ip_address`). Pure acronyms collapse
/// (`UDF` -> `udf`).
fn snake_case(ident: &str) -> String {
    let chars: Vec<char> = ident.chars().collect();
    let mut out = String::with_capacity(ident.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let needs_sep = prev.is_lowercase()
                || (prev.is_uppercase() && next.is_some_and(|n| n.is_lowercase()));
            if needs_sep {
                out.push('_');
            }
        }
        out.extend(c.to_lowercase());
    }
    out
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

/// Returns the explicit discriminant string from `#[codec(rename = "...")]`
/// on a variant, if present.
///
/// Validates that the value is non-empty and contains no `/`, since the
/// discriminant is emitted via `push_raw` (no escaping) and `/` is the
/// segment separator -- a slash in the tag would silently corrupt the
/// key.
fn check_variant_rename(variant: &Variant) -> syn::Result<Option<String>> {
    let mut rename: Option<String> = None;
    for attr in &variant.attrs {
        if !attr.path().is_ident("codec") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                let value = lit.value();
                if value.is_empty() {
                    return Err(meta.error("#[codec(rename = \"...\")] must not be empty"));
                }
                if value.contains('/') {
                    return Err(meta.error(
                        "#[codec(rename = \"...\")] must not contain '/' (segment separator)",
                    ));
                }
                rename = Some(value);
                Ok(())
            } else {
                Err(meta.error("unknown #[codec] option on variant; expected `rename`"))
            }
        })?;
    }
    Ok(rename)
}
