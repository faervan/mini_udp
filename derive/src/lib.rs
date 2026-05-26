use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

#[proc_macro_derive(BitRepr)]
pub fn derive_byte_repr(token_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(token_input as DeriveInput);
    let BitReprImpl {
        min_len,
        max_len,
        f_len,
        f_write_to_bytes,
        f_from_bytes,
    } = match input.data {
        syn::Data::Struct(_) => {
            return compile_error(Span::call_site(), "Derive for struct not yet implemented");
        }
        syn::Data::Enum(data) => match impl_for_enum(data) {
            Ok(v) => v,
            Err(e) => return e,
        },
        syn::Data::Union(_) => {
            return compile_error(Span::call_site(), "Derive for union not yet implemented");
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
            fn write_to_bytes(&self, bytes: &mut [u8], bit_offset: u8) -> Result<(), ::mini_udp::BitReprError> {
                #f_write_to_bytes
            }
            fn from_bytes(bytes: &[u8], bit_offset: u8) -> Result<Self, ::mini_udp::BitReprError> {
                #f_from_bytes
            }
        }
    }
    .into()
}

struct BitReprImpl {
    min_len: usize,
    max_len: usize,
    f_len: TokenStream,
    f_write_to_bytes: TokenStream,
    f_from_bytes: TokenStream,
}

fn impl_for_enum(data: syn::DataEnum) -> Result<BitReprImpl, proc_macro::TokenStream> {
    if let Some(variant) = data.variants.iter().find(|v| !v.attrs.is_empty()) {
        return Err(compile_error(
            variant.span(),
            "Enum attributes are not yet supported",
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
    let len = len.try_into().unwrap();
    Ok(BitReprImpl {
        min_len: len,
        max_len: len,
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
    fn slice_access(&self, slice: TokenStream) -> TokenStream {
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
    fn as_u8(&self) -> TokenStream {
        match self {
            Self::U8 => quote! {},
            Self::U16 | Self::U32 => quote! {.to_le_bytes()},
        }
    }
    fn assign_value(&self, access: TokenStream, value: TokenStream) -> TokenStream {
        match self {
            Self::U8 => quote! {*#access = value},
            Self::U16 | Self::U32 => quote! {#access.copy_from_slice(#value)},
        }
    }
}

fn compile_error(span: Span, msg: impl ToString) -> proc_macro::TokenStream {
    let msg = msg.to_string();
    quote::quote_spanned! {span=> compile_error!(#msg);}.into()
}
