use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{punctuated::Punctuated, token::Comma, Data, DeriveInput, Fields, Variant};

fn field_schema_byte(ty: &syn::Type) -> proc_macro2::TokenStream {
    quote! {
        {
            const _: () = assert!(
                <#ty as ::rapidlog::arg::Encode>::SCHEMA.len() == 1,
                "field schemas with >1 byte not yet supported in derive(Encode)"
            );
            <#ty as ::rapidlog::arg::Encode>::SCHEMA[0]
        }
    }
}

fn variant_schema_byte(ty: &syn::Type) -> proc_macro2::TokenStream {
    quote! {
        {
            const _: () = assert!(
                <#ty as ::rapidlog::arg::Encode>::SCHEMA.len() == 1,
                "variant schemas with >1 byte not yet supported in derive(Encode)"
            );
            <#ty as ::rapidlog::arg::Encode>::SCHEMA[0]
        }
    }
}

fn struct_schema(fields: &syn::FieldsNamed, string_table: &mut Vec<u8>) -> proc_macro2::TokenStream {
    let field_count = fields.named.len();
    let st_idx: u8 = 0;

    let mut field_schema_tokens = Vec::new();
    for field in &fields.named {
        let field_name = field.ident.as_ref().unwrap().to_string();
        let st_off = string_table.len() as u16;
        string_table.extend_from_slice(field_name.as_bytes());
        string_table.push(0);

        let st_off_lo = (st_off & 0xFF) as u8;
        let st_off_hi = (st_off >> 8) as u8;
        let schema_byte = field_schema_byte(&field.ty);

        field_schema_tokens.push(quote! {
            #st_off_lo, #st_off_hi,
            #schema_byte,
        });
    }

    let count_lo = (field_count & 0xFF) as u8;
    let count_hi = ((field_count >> 8) & 0xFF) as u8;

    if field_count <= 15 {
        quote! {
            &[
                0xA0u8 | #field_count as u8,
                #st_idx,
                #(#field_schema_tokens)*
            ]
        }
    } else {
        quote! {
            &[
                0xA0u8,
                #count_lo, #count_hi,
                #st_idx,
                #(#field_schema_tokens)*
            ]
        }
    }
}

fn struct_encode_body(fields: &syn::FieldsNamed) -> proc_macro2::TokenStream {
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

fn struct_max_size_body(fields: &syn::FieldsNamed) -> proc_macro2::TokenStream {
    let mut size_stmts = Vec::new();
    for field in &fields.named {
        let field_name = field.ident.as_ref().unwrap();
        size_stmts.push(quote! {
            + ::rapidlog::arg::Encode::max_encoded_size(&self.#field_name)
        });
    }
    quote! {
        0usize #(#size_stmts)*
    }
}

fn enum_schema(
    variants: &Punctuated<Variant, Comma>,
    string_table: &mut Vec<u8>,
) -> proc_macro2::TokenStream {
    let variant_count = variants.len();
    let st_idx: u8 = 0;
    let vdx_k: u8 = if variant_count <= 255 { 1 } else { 2 };

    let mut var_schema_tokens = Vec::new();
    for variant in variants {
        let variant_name = variant.ident.to_string();
        let st_off = string_table.len() as u16;
        string_table.extend_from_slice(variant_name.as_bytes());
        string_table.push(0);

        let st_off_lo = (st_off & 0xFF) as u8;
        let st_off_hi = (st_off >> 8) as u8;

        match &variant.fields {
            Fields::Unit => {
                var_schema_tokens.push(quote! {
                    #st_off_lo, #st_off_hi, 0x00u8,
                });
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let schema_byte = variant_schema_byte(&fields.unnamed.first().unwrap().ty);
                var_schema_tokens.push(quote! {
                    #st_off_lo, #st_off_hi,
                    #schema_byte,
                });
            }
            _ => {
                var_schema_tokens.push(quote! {
                    #st_off_lo, #st_off_hi, 0x00u8,
                });
            }
        }
    }

    let count_lo = (variant_count & 0xFF) as u8;
    let count_hi = ((variant_count >> 8) & 0xFF) as u8;

    if variant_count <= 15 {
        quote! {
            &[
                0xB0u8 | #variant_count as u8,
                #vdx_k, #st_idx,
                #(#var_schema_tokens)*
            ]
        }
    } else {
        quote! {
            &[
                0xB0u8,
                #count_lo, #count_hi,
                #vdx_k, #st_idx,
                #(#var_schema_tokens)*
            ]
        }
    }
}

fn enum_encode_body(variants: &Punctuated<Variant, Comma>) -> proc_macro2::TokenStream {
    let variant_count = variants.len();
    let uses_u16_discriminant = variant_count > 255;

    let mut arms = Vec::new();
    for (idx, variant) in variants.iter().enumerate() {
        let variant_name = &variant.ident;

        let (encode_discriminant, discriminant_size) = if uses_u16_discriminant {
            let lo = (idx & 0xFF) as u8;
            let hi = (idx >> 8) as u8;
            (
                quote! {
                    buf[0] = #lo;
                    buf[1] = #hi;
                },
                2usize,
            )
        } else {
            let vdx_byte = idx as u8;
            (
                quote! {
                    buf[0] = #vdx_byte;
                },
                1usize,
            )
        };

        match &variant.fields {
            Fields::Unit => {
                arms.push(quote! {
                    Self::#variant_name => {
                        #encode_discriminant
                        #discriminant_size
                    }
                });
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field_names: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| format_ident!("f{}", i))
                    .collect();
                let f0 = &field_names[0];
                arms.push(quote! {
                    Self::#variant_name(#f0) => {
                        #encode_discriminant
                        let used = ::rapidlog::arg::Encode::encode_to(#f0, &mut buf[#discriminant_size..]);
                        #discriminant_size + used
                    }
                });
            }
            _ => {
                arms.push(quote! {
                    Self::#variant_name { .. } => {
                        #encode_discriminant
                        #discriminant_size
                    }
                });
            }
        }
    }
    quote! {
        match self {
            #(#arms)*
        }
    }
}

fn enum_max_size_body(variants: &Punctuated<Variant, Comma>) -> proc_macro2::TokenStream {
    let variant_count = variants.len();
    let discriminant_size: usize = if variant_count > 255 { 2 } else { 1 };

    let mut arms = Vec::new();
    for variant in variants {
        let variant_name = &variant.ident;
        match &variant.fields {
            Fields::Unit => {
                arms.push(quote! {
                    Self::#variant_name => #discriminant_size
                });
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let f0 = format_ident!("f0");
                arms.push(quote! {
                    Self::#variant_name(#f0) => {
                        #discriminant_size + ::rapidlog::arg::Encode::max_encoded_size(#f0)
                    }
                });
            }
            _ => {
                arms.push(quote! {
                    Self::#variant_name { .. } => #discriminant_size
                });
            }
        }
    }
    quote! {
        match self {
            #(#arms)*
        }
    }
}

#[proc_macro_derive(Encode)]
pub fn derive_encode(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let mut string_table = Vec::new();

    let (schema_tokens, encode_body, max_size_body) = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => (
                struct_schema(fields, &mut string_table),
                struct_encode_body(fields),
                struct_max_size_body(fields),
            ),
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "Encode derive only supports named fields for structs",
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
            const SCHEMA: &'static [u8] = #schema_tokens;

            fn encode_to(&self, buf: &mut [u8]) -> usize {
                #encode_body
            }

            fn max_encoded_size(&self) -> usize {
                #max_size_body
            }
        }

        impl ::rapidlog::arg::HasStringTable for #name {
            const STRING_TABLE: &'static [u8] = &[#(#st_bytes),*];
        }
    };

    TokenStream::from(expanded)
}
