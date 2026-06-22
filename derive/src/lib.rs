use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

use crate::{data::DataValue, data_group::Fields, data2::Context};

mod data;
mod data2;
mod data_group;

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
    min_len: TokenStream2,
    max_len: TokenStream2,
    f_len: TokenStream2,
    f_write_to_bytes: TokenStream2,
    f_from_bytes: TokenStream2,
}

fn impl_for_struct(ident: syn::Ident, data: syn::DataStruct) -> Result<BitReprImpl, TokenStream> {
    let mut context = Context::default();
    let values = Fields::new(ident, &data.fields, None, &mut context);
    let read = values.read();
    let write = values.write();

    let fixed_len = context.byte_offset;

    let variable_min_len = values.min_variable_byte_len();
    let min_len = quote! {#fixed_len #variable_min_len};

    let variable_max_len = values.max_variable_byte_len();
    let max_len = quote! {#fixed_len #variable_max_len};

    let variable_len = values.variable_byte_len();

    Ok(BitReprImpl {
        f_len: quote! {#min_len #variable_len},
        min_len,
        max_len,
        f_write_to_bytes: quote! {
            #write
            Ok(())
        },
        f_from_bytes: quote! {Ok({#read})},
    })
}

fn impl_for_enum(ident: syn::Ident, data: syn::DataEnum) -> Result<BitReprImpl, TokenStream> {
    // Get the amount of bits needed to represent all variants
    let variant_bit_len = match data.variants.len() {
        0 | 1 => 0,
        len => (len - 1).ilog2() + 1,
    };
    let variant_bit_len: usize = variant_bit_len.try_into().unwrap();
    let variant_len = (variant_bit_len as f32 / 8.).ceil() as usize;
    let mut min_len = usize::MAX;
    let mut unknown_min_len = vec![];
    let mut max_len = 0;
    // Whether the variant referred to in `max_len` has an unknown part.
    let mut max_is_unknown = false;
    let mut unknown_max_len = vec![];

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
            let fields = Fields::new(
                ident.clone(),
                &variant.fields,
                Some((variant.ident.clone(), id)),
                &mut context,
            );
            (fields, context.byte_offset)
        })
        .collect::<Vec<_>>();
    for (fields, fixed_size) in &variants {
        min_len = min_len.min(*fixed_size);
        let max_variable = fields.max_variable_byte_len();
        if !max_variable.is_empty() {
            unknown_max_len.push(quote! {#fixed_size #max_variable});
            if *fixed_size > max_len {
                max_is_unknown = true;
            }
        } else if *fixed_size > max_len {
            max_is_unknown = false;
        }
        max_len = max_len.max(*fixed_size);
    }
    let read: TokenStream2 = variants
        .iter()
        .enumerate()
        .map(|(i, (fields, _))| {
            let read = fields.read();
            let i = syn::LitInt::new(&format!("{i}{data_type_str}"), Span::mixed_site());
            quote! {#i => {#read}}
        })
        .collect();
    let write: TokenStream2 = variants.iter().map(|(fields, _)| fields.write()).collect();

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
            let read = variants.first().unwrap().0.read();
            quote! {
                Ok(#read)
            }
        }
        false => quote! {
            Ok(match #access {
                #read
                _ => return Err(::mini_udp::BitReprError::InvalidValue),
            })
        },
    };

    let match_len: TokenStream2 = variants
        .iter()
        .map(|(fields, fixed_size)| {
            let binding = fields.binding();
            let variable_size = fields.variable_byte_len();
            quote! {
                #binding => {
                    #fixed_size #variable_size
                }
            }
        })
        .collect();
    let f_len = quote! {
        match self {
            #match_len
        }
    };
    Ok(BitReprImpl {
        min_len,
        max_len,
        f_len,
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
            Self::U8 => quote! {*#access = variant_id},
            Self::U16 | Self::U32 => quote! {#access.copy_from_slice(#value)},
        }
    }
}

/// Represents a struct or enum variant
struct DataValueGroup {
    /// should probably be an enum, whatever
    is_named: bool,
    /// should probably be an enum, whatever
    is_unit: bool,
    enum_variant_id: Option<syn::LitInt>,
    byte_offset: usize,
    ident: TokenStream2,
    values: Vec<DataValue>,
}

impl DataValueGroup {
    fn new(
        fields: &syn::Fields,
        ident: TokenStream2,
        enum_variant_id: Option<syn::LitInt>,
        byte_offset: usize,
    ) -> Self {
        let mut values = vec![];
        let mut is_named = false;
        let mut is_unit = false;
        match fields {
            syn::Fields::Unit => is_unit = true,
            syn::Fields::Named(fields) => {
                values = fields
                    .named
                    .iter()
                    .map(|field| DataValue::from_field(field, None))
                    .collect();
                is_named = true;
            }
            syn::Fields::Unnamed(fields) => {
                values = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(index, field)| DataValue::from_field(field, Some(index)))
                    .collect();
            }
        };
        Self {
            is_named,
            is_unit,
            enum_variant_id,
            byte_offset,
            ident,
            values,
        }
    }

    fn quote_to_bytes(&self) -> TokenStream2 {
        let mut byte_offset = self.byte_offset;
        let write_fixed_to_bytes: TokenStream2 = self
            .values
            .iter()
            .filter(|v| v.data_bytes().is_fixed())
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
        let ident = &self.ident;
        let binding = match (self.is_named, self.is_unit) {
            (true, false) => quote! {#ident { #fields }},
            (false, false) => quote! {#ident ( #fields )},
            (_, true) => quote! {#ident},
        };
        match &self.enum_variant_id {
            Some(id) => quote! {
                #binding => {
                    #write_fixed_to_bytes
                    #id
                }
            },
            None => quote! {{
                let #binding = self;
                #write_fixed_to_bytes
            }},
        }
    }

    fn quote_from_bytes(&self) -> TokenStream2 {
        let ident = &self.ident;
        let mut byte_offset = self.byte_offset;
        let read_fixed_values: TokenStream2 = self
            .values
            .iter()
            .filter(|v| v.data_bytes().is_fixed())
            .map(|v| {
                let identifier = v.quote_identifier(false);
                let value = v.quote_from_bytes(&mut byte_offset);
                quote! {let #identifier = #value;}
            })
            .collect();
        let fields: TokenStream2 = self
            .values
            .iter()
            .map(|v| {
                let field = v.quote_identifier(true);
                quote! {#field,}
            })
            .collect();
        match (self.is_named, self.is_unit) {
            (true, false) => quote! {{
                #read_fixed_values
                #ident { #fields }
            }},
            (false, false) => quote! {{
                #read_fixed_values
                #ident ( #fields )
            }},
            (_, true) => quote! {{#ident}},
        }
    }

    fn header_size(&self) -> usize {
        // TODO!
        1
    }

    fn min_size(&self) -> usize {
        // TODO! Add delegated size
        self.values
            .iter()
            .map(|v| match v.data_bytes() {
                data::FullDataBytes::None => 0,
                data::FullDataBytes::Fixed(s) => s,
                data::FullDataBytes::Range(r) => r.start,
                data::FullDataBytes::Delegated => 0,
            })
            .sum::<usize>()
            + self.header_size()
    }

    fn max_size(&self) -> usize {
        // TODO! Add delegated size
        self.values
            .iter()
            .map(|v| match v.data_bytes() {
                data::FullDataBytes::None => 0,
                data::FullDataBytes::Fixed(s) => s,
                data::FullDataBytes::Range(r) => r.end,
                data::FullDataBytes::Delegated => 0,
            })
            .sum::<usize>()
            + self.header_size()
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
