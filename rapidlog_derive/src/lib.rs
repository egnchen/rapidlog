use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{punctuated::Punctuated, token::Comma, Data, DeriveInput, Fields, Variant};

fn schema_of(ty: &syn::Type) -> proc_macro2::TokenStream {
    quote! {
        <#ty as ::rapidlog::arg::SchemaOf>::schema_of()
    }
}

fn struct_schema_named(
    fields: &syn::FieldsNamed,
    string_table: &mut Vec<u8>,
) -> proc_macro2::TokenStream {
    let field_count = fields.named.len();
    let st_idx: u8 = 0;
    let mut stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    if field_count <= 15 {
        stmts.push(quote! { __v.push(0xA0u8 | #field_count as u8); });
        stmts.push(quote! { __v.push(#st_idx); });
    } else {
        let count_lo = (field_count & 0xFF) as u8;
        let count_hi = ((field_count >> 8) & 0xFF) as u8;
        stmts.push(quote! { __v.push(0xA0u8); });
        stmts.push(quote! { __v.push(#count_lo); });
        stmts.push(quote! { __v.push(#count_hi); });
        stmts.push(quote! { __v.push(#st_idx); });
    }

    for field in &fields.named {
        let field_name = field.ident.as_ref().unwrap().to_string();
        let st_off = string_table.len() as u16;
        string_table.extend_from_slice(field_name.as_bytes());
        string_table.push(0);

        let st_off_lo = (st_off & 0xFF) as u8;
        let st_off_hi = (st_off >> 8) as u8;
        let field_schema = schema_of(&field.ty);

        stmts.push(quote! { __v.push(#st_off_lo); });
        stmts.push(quote! { __v.push(#st_off_hi); });
        stmts.push(quote! { __v.extend_from_slice(#field_schema); });
    }

    quote! { #(#stmts)* }
}

fn struct_schema_unnamed(
    fields: &syn::FieldsUnnamed,
) -> proc_macro2::TokenStream {
    let field_count = fields.unnamed.len();
    let mut stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    if field_count <= 15 {
        stmts.push(quote! { __v.push(0x90u8 | #field_count as u8); });
    } else {
        let count_lo = (field_count & 0xFF) as u8;
        let count_hi = ((field_count >> 8) & 0xFF) as u8;
        stmts.push(quote! { __v.push(0x90u8); });
        stmts.push(quote! { __v.push(#count_lo); });
        stmts.push(quote! { __v.push(#count_hi); });
    }

    for field in &fields.unnamed {
        let field_schema = schema_of(&field.ty);
        stmts.push(quote! { __v.extend_from_slice(#field_schema); });
    }

    quote! { #(#stmts)* }
}

fn struct_encode_body_named(fields: &syn::FieldsNamed) -> proc_macro2::TokenStream {
    let mut encode_stmts = Vec::new();
    for field in &fields.named {
        let field_name = field.ident.as_ref().unwrap();
        encode_stmts.push(quote! {
            pos += ::rapidlog::arg::Encode::encode_to(&self.#field_name, &mut buf[pos..]);
        });
    }
    quote! {
        let mut pos = 0usize;
        #(#encode_stmts)*
        pos
    }
}

fn struct_encode_body_unnamed(fields: &syn::FieldsUnnamed) -> proc_macro2::TokenStream {
    let field_names: Vec<_> = (0..fields.unnamed.len())
        .map(|i| format_ident!("f{}", i))
        .collect();
    let mut encode_stmts = Vec::new();
    for fname in &field_names {
        encode_stmts.push(quote! {
            pos += ::rapidlog::arg::Encode::encode_to(#fname, &mut buf[pos..]);
        });
    }
    quote! {
        let mut pos = 0usize;
        #(#encode_stmts)*
        pos
    }
}

fn struct_max_size_body_named(fields: &syn::FieldsNamed) -> proc_macro2::TokenStream {
    let mut size_stmts = Vec::new();
    for field in &fields.named {
        let field_name = field.ident.as_ref().unwrap();
        size_stmts.push(quote! {
            + ::rapidlog::arg::Encode::max_encoded_size(&self.#field_name)
        });
    }
    quote! { 0usize #(#size_stmts)* }
}

fn struct_max_size_body_unnamed(fields: &syn::FieldsUnnamed) -> proc_macro2::TokenStream {
    let field_names: Vec<_> = (0..fields.unnamed.len())
        .map(|i| format_ident!("f{}", i))
        .collect();
    let mut size_stmts = Vec::new();
    for fname in &field_names {
        size_stmts.push(quote! {
            + ::rapidlog::arg::Encode::max_encoded_size(#fname)
        });
    }
    quote! { 0usize #(#size_stmts)* }
}

fn variant_sub_schema(
    fields: &Fields,
    variant_string_table: &mut Vec<u8>,
) -> proc_macro2::TokenStream {
    match fields {
        Fields::Unit => {
            quote! { __v.push(0x00u8); }
        }
        Fields::Unnamed(f) if f.unnamed.len() == 1 => {
            let field_schema = schema_of(&f.unnamed.first().unwrap().ty);
            quote! { __v.extend_from_slice(#field_schema); }
        }
        Fields::Unnamed(f) => {
            struct_schema_unnamed(f)
        }
        Fields::Named(f) => {
            struct_schema_named(f, variant_string_table)
        }
    }
}

fn variant_encode_body(
    variant: &Variant,
    idx: usize,
    uses_u16: bool,
) -> proc_macro2::TokenStream {
    let variant_name = &variant.ident;
    let (encode_disc, disc_size) = if uses_u16 {
        let lo = (idx & 0xFF) as u8;
        let hi = (idx >> 8) as u8;
        (
            quote! { buf[0] = #lo; buf[1] = #hi; },
            2usize,
        )
    } else {
        let vdx = idx as u8;
        (quote! { buf[0] = #vdx; }, 1usize)
    };

    match &variant.fields {
        Fields::Unit => {
            quote! {
                Self::#variant_name => {
                    #encode_disc
                    #disc_size
                }
            }
        }
        Fields::Unnamed(f) => {
            let field_names: Vec<_> = (0..f.unnamed.len())
                .map(|i| format_ident!("__f{}", i))
                .collect();
            let bindings = &field_names;
            let enc_stmts: Vec<_> = bindings
                .iter()
                .map(|name| {
                    quote! {
                        pos += ::rapidlog::arg::Encode::encode_to(#name, &mut buf[pos..]);
                    }
                })
                .collect();
            quote! {
                Self::#variant_name(#(#bindings),*) => {
                    #encode_disc
                    let mut pos = #disc_size;
                    #(#enc_stmts)*
                    pos
                }
            }
        }
        Fields::Named(f) => {
            let field_names: Vec<_> = f.named.iter()
                .map(|field| field.ident.as_ref().unwrap())
                .collect();
            let bindings = &field_names;
            let enc_stmts: Vec<_> = bindings.iter().map(|name| {
                quote! {
                    pos += ::rapidlog::arg::Encode::encode_to(#name, &mut buf[pos..]);
                }
            }).collect();
            quote! {
                Self::#variant_name { #(#bindings),* } => {
                    #encode_disc
                    let mut pos = #disc_size;
                    #(#enc_stmts)*
                    pos
                }
            }
        }
    }
}

fn variant_max_size_body(variant: &Variant, disc_size: usize) -> proc_macro2::TokenStream {
    let variant_name = &variant.ident;
    match &variant.fields {
        Fields::Unit => {
            quote! { Self::#variant_name => #disc_size }
        }
        Fields::Unnamed(f) => {
            let field_names: Vec<_> = (0..f.unnamed.len())
                .map(|i| format_ident!("__f{}", i))
                .collect();
            let sum: Vec<_> = field_names.iter().map(|name| {
                quote! { + ::rapidlog::arg::Encode::max_encoded_size(#name) }
            }).collect();
            quote! {
                Self::#variant_name(#(#field_names),*) => {
                    #disc_size #(#sum)*
                }
            }
        }
        Fields::Named(f) => {
            let field_names: Vec<_> = f.named.iter()
                .map(|field| field.ident.as_ref().unwrap())
                .collect();
            let sum: Vec<_> = field_names.iter().map(|name| {
                quote! { + ::rapidlog::arg::Encode::max_encoded_size(#name) }
            }).collect();
            quote! {
                Self::#variant_name { #(#field_names),* } => {
                    #disc_size #(#sum)*
                }
            }
        }
    }
}

fn enum_schema(
    variants: &Punctuated<Variant, Comma>,
    string_table: &mut Vec<u8>,
) -> proc_macro2::TokenStream {
    let variant_count = variants.len();
    let st_idx: u8 = 0;
    let vdx_k: u8 = if variant_count <= 255 { 1 } else { 2 };
    let mut stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    if variant_count <= 15 {
        stmts.push(quote! { __v.push(0xB0u8 | #variant_count as u8); });
        stmts.push(quote! { __v.push(#vdx_k); });
        stmts.push(quote! { __v.push(#st_idx); });
    } else {
        let count_lo = (variant_count & 0xFF) as u8;
        let count_hi = ((variant_count >> 8) & 0xFF) as u8;
        stmts.push(quote! { __v.push(0xB0u8); });
        stmts.push(quote! { __v.push(#count_lo); });
        stmts.push(quote! { __v.push(#count_hi); });
        stmts.push(quote! { __v.push(#vdx_k); });
        stmts.push(quote! { __v.push(#st_idx); });
    }

    for variant in variants {
        let variant_name = variant.ident.to_string();
        let st_off = string_table.len() as u16;
        string_table.extend_from_slice(variant_name.as_bytes());
        string_table.push(0);

        let st_off_lo = (st_off & 0xFF) as u8;
        let st_off_hi = (st_off >> 8) as u8;
        let sub_schema = variant_sub_schema(&variant.fields, string_table);

        stmts.push(quote! { __v.push(#st_off_lo); });
        stmts.push(quote! { __v.push(#st_off_hi); });
        stmts.push(sub_schema);
    }

    quote! { #(#stmts)* }
}

#[proc_macro_derive(Encode)]
pub fn derive_encode(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let mut string_table = Vec::new();

    let (schema_body, encode_body, max_size_body) = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => (
                struct_schema_named(fields, &mut string_table),
                struct_encode_body_named(fields),
                struct_max_size_body_named(fields),
            ),
            Fields::Unnamed(fields) => (
                struct_schema_unnamed(fields),
                struct_encode_body_unnamed(fields),
                struct_max_size_body_unnamed(fields),
            ),
            Fields::Unit => {
                return syn::Error::new_spanned(
                    &input,
                    "Encode derive does not support unit structs",
                )
                .to_compile_error()
                .into();
            }
        },
        Data::Enum(data) => (
            enum_schema(&data.variants, &mut string_table),
            enum_encode_body(&data.variants),
            enum_max_size_body(&data.variants),
        ),
        Data::Union(_) => {
            return syn::Error::new_spanned(&input, "Encode derive does not support unions")
                .to_compile_error()
                .into();
        }
    };

    let st_bytes = string_table;

    let expanded = quote! {
        impl ::rapidlog::arg::Encode for #name {
            fn schema() -> &'static [u8] {
                use std::sync::OnceLock;
                static SCHEMA: OnceLock<Vec<u8>> = OnceLock::new();
                SCHEMA.get_or_init(|| {
                    let mut __v = Vec::new();
                    #schema_body
                    __v
                }).as_slice()
            }

            fn encode_to(&self, buf: &mut [u8]) -> usize {
                #encode_body
            }

            fn max_encoded_size(&self) -> usize {
                #max_size_body
            }

            fn string_table(&self) -> &'static [u8] {
                <Self as ::rapidlog::arg::HasStringTable>::STRING_TABLE
            }
        }

        impl ::rapidlog::arg::HasStringTable for #name {
            const STRING_TABLE: &'static [u8] = &[#(#st_bytes),*];
        }
    };

    TokenStream::from(expanded)
}

fn enum_encode_body(variants: &Punctuated<Variant, Comma>) -> proc_macro2::TokenStream {
    let variant_count = variants.len();
    let uses_u16 = variant_count > 255;
    let arms: Vec<_> = variants
        .iter()
        .enumerate()
        .map(|(idx, v)| variant_encode_body(v, idx, uses_u16))
        .collect();
    quote! { match self { #(#arms)* } }
}

fn enum_max_size_body(variants: &Punctuated<Variant, Comma>) -> proc_macro2::TokenStream {
    let variant_count = variants.len();
    let disc_size: usize = if variant_count > 255 { 2 } else { 1 };
    let arms: Vec<_> = variants
        .iter()
        .map(|v| variant_max_size_body(v, disc_size))
        .collect();
    quote! { match self { #(#arms)* } }
}
