use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens as _, quote};
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

use crate::data::DataValue;

mod data;

#[proc_macro_derive(BitRepr)]
pub fn derive_byte_repr(token_input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(token_input as DeriveInput);
    let BitReprImpl {
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
        syn::Data::Enum(data) => match impl_for_enum(data) {
            Ok(v) => v,
            Err(e) => return e,
        },
        syn::Data::Union(_) => {
            return compile_error(Span::call_site(), "Derive for union is not yet implemented");
        }
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let ident = input.ident;
    quote! {
        impl #impl_generics ::mini_udp::BitRepr for #ident #ty_generics #where_clause {
            const MIN_BIT_LEN: usize = #min_len;
            const MAX_BIT_LEN: usize = #max_len;
            fn bit_len(&self) -> usize {
                #f_len
            }
            fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<(), ::mini_udp::BitReprError> {
                #f_write_to_bytes
            }
            fn from_bytes(bytes: &[u8]) -> Result<Self, ::mini_udp::BitReprError> {
                #f_from_bytes
            }
        }
    }
    .into()
}

struct BitReprImpl {
    min_len: usize,
    max_len: usize,
    f_len: TokenStream2,
    f_write_to_bytes: TokenStream2,
    f_from_bytes: TokenStream2,
}

fn impl_for_struct(ident: syn::Ident, data: syn::DataStruct) -> Result<BitReprImpl, TokenStream> {
    let values = DataValueGroup::new(&data.fields, ident, false, 0);
    let to_bytes = values.quote_to_bytes();
    let from_bytes = values.quote_from_bytes();

    Ok(BitReprImpl {
        // TODO!
        min_len: 0,
        // TODO!
        max_len: 0,
        f_len: quote! {todo!()},
        f_write_to_bytes: quote! {
            #to_bytes
            Ok(())
        },
        f_from_bytes: quote! {Ok(#from_bytes)},
    })
}

fn impl_for_enum(data: syn::DataEnum) -> Result<BitReprImpl, TokenStream> {
    let mut min_field_len = usize::MAX;
    let mut max_field_len = 0;
    let variants = data
        .variants
        .iter()
        .map(|variant| FieldBitReprImpl::new(&variant.ident, &variant.fields))
        .collect::<Result<Vec<_>, TokenStream>>()?;
    for variant in &variants {
        min_field_len = min_field_len.min(variant.size);
        max_field_len = max_field_len.max(variant.size);
    }
    if let Some(variant) = data.variants.iter().find(|v| !v.fields.is_empty()) {
        return Err(compile_error(
            variant.span(),
            format!(
                "variant {} has fields: min enum len: {min_field_len}, max enum len: {max_field_len}",
                variant.ident
            ),
        ));
    }
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
    let mut match_to_bytes = quote! {};
    let mut match_from_bytes = quote! {};
    for (i, v) in data.variants.iter().enumerate() {
        let variant = v.clone();
        let i = syn::LitInt::new(&format!("{i}{data_type_str}"), Span::call_site());
        match_to_bytes = quote! {
            #match_to_bytes
            Self::#variant => #i,
        };
        match_from_bytes = quote! {
            #match_from_bytes
            #i => Self::#variant,
        };
    }

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
        .assign_value(access, quote! {value #to_bytes});
    let f_write_to_bytes = quote! {
        let value = match self {
            #match_to_bytes
        };
        #assign;
        Ok(())
    };

    let data_access = DataAccess {
        ty: data_access_type,
        mutable: false,
        byte_offset: 0,
    };
    let access = data_access.slice_access(bytes_ident);
    let f_from_bytes = quote! {
        Ok(match #access {
            #match_from_bytes
            _ => return Err(::mini_udp::BitReprError::InvalidValue),
        })
    };
    // Get the amount of bits needed to represent all variants
    let len = match data.variants.len() {
        0 | 1 => 0,
        len => (len - 1).ilog2() + 1,
    };
    let len: usize = len.try_into().unwrap();
    Ok(BitReprImpl {
        min_len: len + min_field_len,
        max_len: len + max_field_len,
        f_len: quote! {#len},
        f_write_to_bytes,
        f_from_bytes,
    })
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
            DataAccessType::U8 => quote! {.ok_or(::mini_udp::BitReprError::SliceTooShort)},
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
            Self::U8 => quote! {*#access = value},
            Self::U16 | Self::U32 => quote! {#access.copy_from_slice(#value)},
        }
    }
}

struct FieldBitReprImpl {
    size: usize,
    f_write_to_bytes: TokenStream2,
    f_from_bytes: TokenStream2,
}

impl FieldBitReprImpl {
    fn new(ident: &syn::Ident, fields: &syn::Fields) -> Result<Self, TokenStream> {
        let size = get_fields_size(fields)?;
        let f_write_to_bytes = quote! {todo!()};
        let f_from_bytes = quote! {todo!()};
        Ok(Self {
            size,
            f_write_to_bytes,
            f_from_bytes,
        })
    }
}

fn get_fields_size(fields: &syn::Fields) -> Result<usize, TokenStream> {
    let fields = match fields {
        syn::Fields::Unit => return Ok(0),
        syn::Fields::Named(fields) => &fields.named,
        syn::Fields::Unnamed(fields) => &fields.unnamed,
    };
    let mut size = 0;
    for field in fields {
        size += get_type_size(&field.ty)?;
    }
    Ok(size)
}

fn get_type_size(ty: &syn::Type) -> Result<usize, TokenStream> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .get_ident()
            .and_then(get_type_path_size)
            .ok_or_else(|| compile_error(ty.span(), "Type not yet supported for derived BitRepr")),
        _ => Err(compile_error(
            ty.span(),
            "Type not yet supported for derived BitRepr",
        )),
    }
}

fn get_type_path_size(ident: &syn::Ident) -> Option<usize> {
    Some(match ident.to_string().as_str() {
        "bool" => 1,
        "u8" | "i8" => 8,
        "u16" | "i16" => 16,
        "u32" | "i32" | "f32" => 32,
        "u64" | "i64" | "f64" => 64,
        "u128" | "i128" => 128,
        #[cfg(target_pointer_width = "32")]
        "usize" | "isize" => 32,
        #[cfg(target_pointer_width = "64")]
        "usize" | "isize" => 64,
        _ => return None,
    })
}

fn surround_fields(fields: &syn::Fields, inner: TokenStream2) -> TokenStream2 {
    match fields {
        syn::Fields::Unit => inner,
        syn::Fields::Unnamed(_) => quote! {(#inner)},
        syn::Fields::Named(_) => quote! {{
            #inner
        }},
    }
}

/// Represents a struct, union or enum variant
struct DataValueGroup {
    /// should be an enum not a bool, whatever
    is_named: bool,
    /// should be an enum not a bool, whatever
    is_union: bool,
    is_enum_variant: bool,
    byte_offset: usize,
    ident: syn::Ident,
    values: Vec<DataValue>,
}

impl DataValueGroup {
    fn new(
        fields: &syn::Fields,
        ident: syn::Ident,
        is_enum_variant: bool,
        byte_offset: usize,
    ) -> Self {
        let mut values = vec![];
        let (is_named, is_union) = match fields {
            syn::Fields::Unit => (false, true),
            syn::Fields::Named(fields) => {
                values = fields
                    .named
                    .iter()
                    .map(|field| DataValue::from_field(field, None))
                    .collect();
                (true, false)
            }
            syn::Fields::Unnamed(fields) => {
                values = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(index, field)| DataValue::from_field(field, Some(index)))
                    .collect();
                (false, false)
            }
        };
        Self {
            is_named,
            is_union,
            is_enum_variant,
            byte_offset,
            ident,
            values,
        }
    }

    fn quote_to_bytes(&self) -> TokenStream2 {
        if self.is_union {
            return quote! {};
        }
        let mut byte_offset = self.byte_offset;
        let write_to_bytes: TokenStream2 = self
            .values
            .iter()
            .map(|v| v.quote_to_bytes(&mut byte_offset))
            .collect();
        let fields: TokenStream2 = self
            .values
            .iter()
            .map(|v| {
                let field_ident = v.quote_identifier(true);
                quote! {#field_ident,}
            })
            .collect();
        let ident = self.ident.to_token_stream();
        let binding = match self.is_named {
            true => quote! {#ident { #fields }},
            false => quote! {#ident ( #fields )},
        };
        match self.is_enum_variant {
            true => quote! {
                #binding => {
                    #write_to_bytes
                }
            },
            false => quote! {
                let #binding = self;
                #write_to_bytes
            },
        }
    }

    fn quote_from_bytes(&self) -> TokenStream2 {
        let ident = self.ident.to_token_stream();
        if self.is_union {
            return ident;
        }
        let mut byte_offset = self.byte_offset;
        let values: TokenStream2 = self
            .values
            .iter()
            .map(|v| {
                let value = v.quote_from_bytes(&mut byte_offset);
                quote! {#value,}
            })
            .collect();
        match self.is_named {
            true => quote! {
                #ident { #values }
            },
            false => quote! {
                #ident ( #values )
            },
        }
    }
}

struct HeaderBits {
    bit_len: usize,
    bytes: HeaderBitBytes,
}

impl HeaderBits {
    fn from_values(values: Vec<DataValue>) -> Self {
        let bit_len = values.iter().map(|v| v.header_bits()).sum();
        Self {
            bit_len,
            bytes: match bit_len {
                _ if bit_len == 0 => HeaderBitBytes::None,
                v if v <= 8 => HeaderBitBytes::U8,
                v if v <= 16 => HeaderBitBytes::U16,
                v if v <= 32 => HeaderBitBytes::U32,
                v if v <= 64 => HeaderBitBytes::U64,
                v if v <= 128 => HeaderBitBytes::U64,
                _ => todo!(),
            },
        }
    }

    fn quote_to_bytes(&self) -> TokenStream2 {
        let ty = match self.bytes {
            HeaderBitBytes::None => return quote! {},
            HeaderBitBytes::U8 => "u8",
            HeaderBitBytes::U16 => "u16",
            HeaderBitBytes::U32 => "u32",
            HeaderBitBytes::U64 => "u64",
            HeaderBitBytes::U128 => "128",
        };
        quote! {
            let mut header_bits: #ty = 0;
        }
    }
}

enum HeaderBitBytes {
    None,
    U8,
    U16,
    U32,
    U64,
    U128,
}

fn compile_error(span: Span, msg: impl ToString) -> TokenStream {
    let msg = msg.to_string();
    quote::quote_spanned! {span=> compile_error!(#msg);}.into()
}
