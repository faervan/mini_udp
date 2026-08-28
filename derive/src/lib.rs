use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::{DeriveInput, parse_macro_input};

use crate::{data::Context, data_group::Fields};

mod data;
mod data_group;

#[proc_macro_derive(ByteRepr)]
/// See the trait docs: [`ByteRepr`](https://docs.rs/mini_udp/latest/mini_udp/trait.ByteRepr.html)
pub fn derive_byte_repr(token_input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(token_input as DeriveInput);
    let ByteReprImpl {
        is_static,
        min_len,
        max_len,
        f_len,
        f_write_to_bytes,
        f_from_bytes,
    } = match input.data {
        syn::Data::Struct(data) => match impl_for_struct(input.ident.clone(), data) {
            Ok(v) => v,
            Err(e) => return e,
        },
        syn::Data::Enum(data) => match impl_for_enum(input.ident.clone(), data) {
            Ok(v) => v,
            Err(e) => return e,
        },
        syn::Data::Union(_) => {
            return compile_error(Span::call_site(), "Derive for union is not yet implemented");
        }
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let ident = input.ident;
    let impl_static = if is_static {
        quote! {
            impl #impl_generics ::mini_udp::StaticByteRepr for #ident #ty_generics #where_clause {
                const BYTE_LEN: usize = #max_len;
            }
        }
    } else {
        quote! {}
    };
    quote! {
        #impl_static
        impl #impl_generics ::mini_udp::ByteRepr for #ident #ty_generics #where_clause {
            #[allow(clippy::int_plus_one)]
            const MIN_BYTE_LEN: usize = #min_len;
            #[allow(clippy::int_plus_one)]
            const MAX_BYTE_LEN: usize = #max_len;
            fn byte_len(&self) -> usize {
                #f_len
            }
            fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ::mini_udp::ByteReprError> {
                #f_write_to_bytes
            }
            fn from_bytes(bytes: &[u8]) -> Result<Self, ::mini_udp::ByteReprError> {
                #f_from_bytes
            }
        }
    }
    .into()
}

struct ByteReprImpl {
    is_static: bool,
    min_len: TokenStream2,
    max_len: TokenStream2,
    f_len: TokenStream2,
    f_write_to_bytes: TokenStream2,
    f_from_bytes: TokenStream2,
}

fn impl_for_struct(ident: syn::Ident, data: syn::DataStruct) -> Result<ByteReprImpl, TokenStream> {
    let mut context = Context::default();
    let values = Fields::new(ident, &data.fields, None, &mut context);
    let read = values.read();
    let write = values.write();
    let length = values.length();

    Ok(ByteReprImpl {
        is_static: length.is_static(),
        f_len: length.as_length(),
        min_len: length.as_const_min_length(),
        max_len: length.as_const_max_length(),
        f_write_to_bytes: quote! {
            #write
            Ok(())
        },
        f_from_bytes: quote! {Ok({#read})},
    })
}

fn impl_for_enum(ident: syn::Ident, data: syn::DataEnum) -> Result<ByteReprImpl, TokenStream> {
    // Get the amount of bits needed to represent all variants
    let variant_byte_len = match data.variants.len() {
        0 | 1 => 0,
        len => (len - 1).ilog2() + 1,
    };
    let variant_byte_len: usize = variant_byte_len.try_into().unwrap();
    let variant_len = (variant_byte_len as f32 / 8.).ceil() as usize;

    let data_access_type = match data.variants.len() {
        0 => {
            return Err(compile_error(
                data.enum_token.span,
                "Enums without variants are not supported",
            ));
        }
        len if len <= 2_usize.pow(8) => DataAccessType::U8,
        len if len <= 2_usize.pow(16) => DataAccessType::U16,
        len if len <= 2_usize.pow(32) => DataAccessType::U32,
        _ => return Err(compile_error(data.enum_token.span, "What are you doing?")),
    };
    let data_type_str = data_access_type.as_str();
    // Stores the fields and their fixed size for each variant
    let variants = data
        .variants
        .iter()
        .enumerate()
        .map(|(id, variant)| {
            let mut context = Context {
                byte_offset: variant_len,
            };
            let id = syn::LitInt::new(&format!("{id}{data_type_str}"), Span::mixed_site());
            Fields::new(
                ident.clone(),
                &variant.fields,
                Some((variant.ident.clone(), id)),
                &mut context,
            )
        })
        .collect::<Vec<_>>();
    let read: TokenStream2 = variants
        .iter()
        .enumerate()
        .map(|(i, fields)| {
            let read = fields.read();
            let i = syn::LitInt::new(&format!("{i}{data_type_str}"), Span::mixed_site());
            quote! {#i => {#read}}
        })
        .collect();
    let write: TokenStream2 = variants.iter().map(Fields::write).collect();

    let bytes_ident = quote! {bytes};
    let data_access_mut = DataAccess {
        ty: data_access_type,
        mutable: true,
        byte_offset: 0,
    };
    let access = data_access_mut.slice_access(bytes_ident.clone());
    let to_bytes = data_access_mut.ty.as_u8();
    let assign = data_access_mut
        .ty
        .assign_value(access, quote! {variant_id #to_bytes});
    let f_write_to_bytes = match variants.len() <= 1 {
        true => quote! {
            let _ = match self {
                #write
            };
            Ok(())
        },
        false => quote! {
            let variant_id = match self {
                #write
            };
            #assign;
            Ok(())
        },
    };

    let data_access = DataAccess {
        ty: data_access_type,
        mutable: false,
        byte_offset: 0,
    };
    let access = data_access.slice_access(bytes_ident);
    let f_from_bytes = match variants.len() == 1 {
        true => {
            let read = variants.first().unwrap().read();
            quote! {
                Ok({#read})
            }
        }
        false => quote! {
            Ok(match #access {
                #read
                _ => return Err(::mini_udp::ByteReprError::InvalidValue),
            })
        },
    };

    let match_len: TokenStream2 = variants
        .iter()
        .map(|fields| {
            let binding = fields.binding();
            let length = fields.length().as_length();
            quote! {
                #binding => {
                    #length
                }
            }
        })
        .collect();
    let f_len = quote! {
        (match self {
            #match_len
        }) + #variant_len
    };
    Ok(ByteReprImpl {
        is_static: variants.iter().all(|f| f.length().is_static()),
        min_len: lowest_min_len(&variants, variant_len),
        max_len: lowest_max_len(&variants, variant_len),
        f_len,
        f_write_to_bytes,
        f_from_bytes,
    })
}

fn lowest_min_len(variants: &[Fields], variant_len: usize) -> TokenStream2 {
    let known_min = variants
        .iter()
        .map(|f| f.length())
        .filter(|f| f.unknown_min.is_empty())
        .map(|f| f.known_min_length())
        .min()
        .unwrap_or(usize::MAX);
    if variants.iter().all(|f| f.length().unknown_min.is_empty()) {
        (known_min + variant_len).to_token_stream()
    } else {
        let mut options = vec![known_min.into_token_stream()];
        for length in variants.iter().map(Fields::length) {
            if length.unknown_min.is_empty() {
                continue;
            }
            options.push(length.as_const_min_length());
        }
        let min_variant = const_min_max_list(&options, quote! {<=});
        quote! {#min_variant + #variant_len}
    }
}

fn lowest_max_len(variants: &[Fields], variant_len: usize) -> TokenStream2 {
    let known_max = variants
        .iter()
        .map(|f| f.length())
        .filter(|f| f.unknown_max.is_empty())
        .map(|f| f.known_max_length())
        .max()
        .unwrap_or_default();
    if variants.iter().all(|f| f.length().unknown_max.is_empty()) {
        (known_max + variant_len).to_token_stream()
    } else {
        let mut options = vec![known_max.to_token_stream()];
        for length in variants.iter().map(Fields::length) {
            if length.unknown_max.is_empty() {
                continue;
            }
            options.push(length.as_const_max_length());
        }
        let max_variant = const_min_max_list(&options, quote! {>=});
        quote! {#max_variant + #variant_len}
    }
}

fn const_min_max_list(options: &[TokenStream2], operator: TokenStream2) -> TokenStream2 {
    assert!(!options.is_empty());
    let first = options[0].clone();
    if options.len() == 1 {
        return first;
    }
    let checks = options
        .iter()
        .skip(1)
        .enumerate()
        .map(|(i, len)| {
            if i + 2 < options.len() {
                quote! {#first #operator #len && }
            } else {
                quote! {#first #operator #len}
            }
        })
        .collect::<TokenStream2>();
    let nest = const_min_max_list(&options[1..], operator);
    quote! {
        if #checks {
            #first
        } else {
            #nest
        }
    }
}

struct DataAccess {
    ty: DataAccessType,
    mutable: bool,
    byte_offset: usize,
}

impl DataAccess {
    fn slice_access(&self, slice: TokenStream2) -> TokenStream2 {
        let byte = self.byte_offset;
        let access = match (self.ty, self.mutable) {
            (DataAccessType::U8, false) => quote! {#slice.get(#byte)},
            (DataAccessType::U8, true) => quote! {#slice.get_mut(#byte)},
            (DataAccessType::U16, false) => quote! {&#slice[#byte..#byte + 2]},
            (DataAccessType::U16, true) => quote! {&mut #slice[#byte..#byte + 2]},
            (DataAccessType::U32, false) => quote! {&#slice[#byte..#byte + 4]},
            (DataAccessType::U32, true) => quote! {&mut #slice[#byte..#byte + 4]},
        };
        let handle_err = match self.ty {
            DataAccessType::U8 => quote! {.ok_or(::mini_udp::ByteReprError::SliceTooShort)},
            DataAccessType::U16 | DataAccessType::U32 => quote! {.try_into()},
        };
        quote! {
            #access #handle_err?
        }
    }
}

#[derive(Clone, Copy)]
enum DataAccessType {
    U8,
    U16,
    U32,
}

impl DataAccessType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
        }
    }
    fn as_u8(&self) -> TokenStream2 {
        match self {
            Self::U8 => quote! {},
            Self::U16 | Self::U32 => quote! {.to_le_bytes()},
        }
    }
    fn assign_value(&self, access: TokenStream2, value: TokenStream2) -> TokenStream2 {
        match self {
            Self::U8 => quote! {*#access = variant_id},
            Self::U16 | Self::U32 => quote! {#access.copy_from_slice(#value)},
        }
    }
}

struct DeriveForInput {
    ty: syn::Type,
    generics: syn::Generics,
    where_clause: Option<syn::WhereClause>,
}

impl syn::parse::Parse for DeriveForInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ty = syn::Type::parse(input)?;
        let generics = if input.is_empty() {
            syn::Generics::default()
        } else {
            input.parse::<syn::Token![,]>()?;
            syn::Generics::parse(input)?
        };
        let where_clause = if input.is_empty() {
            None
        } else {
            Some(syn::WhereClause::parse(input)?)
        };
        Ok(Self {
            ty,
            generics,
            where_clause,
        })
    }
}

#[doc(hidden)]
#[proc_macro]
pub fn derive_for(token_input: TokenStream) -> TokenStream {
    let DeriveForInput {
        ty,
        generics,
        where_clause,
    } = parse_macro_input!(token_input as DeriveForInput);
    let ty_ident = ty.to_token_stream();

    let ident = data::FieldIdentifier::None;
    let ident_binding = ident.as_tokens(true);
    let mut context = Context::default();
    let value = data::Field::new(&mut context, ident, data::Value::from_type(&ty), true);
    let byte_ptr = context.byte_offset;
    let read_fixed = value.read_fixed_part();
    let read_variable = value.read_variable_part();
    let write_fixed = value.write_fixed_part();
    let write_variable = value.write_variable_part();
    let length = value.length();
    let min_len = length.as_const_min_length();
    let max_len = length.as_const_max_length();
    let f_len = length.as_length();

    let impl_static = if length.is_static() {
        quote! {
            impl #generics ::mini_udp::StaticByteRepr for #ty_ident #where_clause {
                const BYTE_LEN: usize = #max_len;
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #impl_static

        impl #generics ::mini_udp::ByteRepr for #ty_ident #where_clause {
            const MIN_BYTE_LEN: usize = #min_len;
            const MAX_BYTE_LEN: usize = #max_len;
            fn byte_len(&self) -> usize {
                #f_len
            }
            fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ::mini_udp::ByteReprError> {
                #[cfg(debug_assertions)]
                if cfg!(debug_assertions) && Self::MIN_BYTE_LEN == 0 {
                    ::mini_udp::tracing::warn!("Serializing a zero-sized type is not meaningful!");
                }
                let #ident_binding = self;
                #write_fixed
                let mut byte_ptr = #byte_ptr;
                #write_variable
                Ok(())
            }
            fn from_bytes(bytes: &[u8]) -> Result<Self, ::mini_udp::ByteReprError> {
                #[cfg(debug_assertions)]
                if cfg!(debug_assertions) && Self::MIN_BYTE_LEN == 0 {
                    ::mini_udp::tracing::warn!("Deserializing a zero-sized type is not meaningful!");
                }
                #read_fixed
                let mut byte_ptr = #byte_ptr;
                #read_variable
                Ok(#ident_binding)
            }
        }
    }.into()
}

fn compile_error(span: Span, msg: impl ToString) -> TokenStream {
    let msg = msg.to_string();
    quote::quote_spanned! {span=> compile_error!(#msg);}.into()
}
