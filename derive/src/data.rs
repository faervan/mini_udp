use std::ops::{Deref, Range};

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens as _, quote};

#[derive(Debug, PartialEq)]
pub struct DataValue {
    kind: DataValueKind,
    ident: DataIdentifier,
}

impl DataValue {
    pub fn from_field(field: &syn::Field, index: Option<usize>) -> Self {
        let kind = DataValueKind::from_type(&field.ty);
        let ident = field.ident.clone().map_or_else(
            || index.map(DataIdentifier::Unnamed).unwrap(),
            DataIdentifier::Named,
        );
        Self { kind, ident }
    }

    /// `real_named_prefix` determines if an identifier for reference should be returned:
    /// `ident = value;`
    /// or if a binding should be created instead, where the field will be renamed for named
    /// variants:
    /// `let DemoStruct { field: named_field } = value;`
    pub fn quote_identifier(&self, real_named_prefix: bool) -> TokenStream2 {
        match &self.ident {
            DataIdentifier::Named(ident) => {
                let prefix = match real_named_prefix {
                    true => quote! {#ident : },
                    false => quote! {},
                };
                let identifier = syn::Ident::new(&format!("named_{ident}"), Span::mixed_site());
                quote! {#prefix #identifier}
            }
            DataIdentifier::Unnamed(index) => {
                syn::Ident::new(&format!("unnamed_{index}"), Span::mixed_site()).to_token_stream()
            }
        }
    }

    pub fn quote_to_bytes(&self, byte_offset: &mut usize) -> TokenStream2 {
        let value = self.quote_identifier(false);
        match self.kind {
            DataValueKind::U8 => {
                let byte = *byte_offset;
                *byte_offset += 1;
                quote! {
                    if let Some(byte) = bytes.get_mut(#byte) {
                        *byte = *#value;
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            DataValueKind::I8 => {
                let byte = *byte_offset;
                *byte_offset += 1;
                quote! {
                    if let Some(byte) = bytes.get_mut(#byte) {
                        *byte = *#value as u8;
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            DataValueKind::F32 => {
                let start = *byte_offset;
                *byte_offset += 4;
                quote! {
                    if let Some(slice) = bytes.get_mut(#start..#byte_offset) {
                        slice.copy_from_slice(&#value.to_le_bytes());
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            DataValueKind::F64 => {
                let start = *byte_offset;
                *byte_offset += 8;
                quote! {
                    if let Some(slice) = bytes.get_mut(#start..#byte_offset) {
                        slice.copy_from_slice(&#value.to_le_bytes());
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            _ => todo!(),
        }
    }

    pub fn quote_from_bytes(&self, byte_offset: &mut usize) -> TokenStream2 {
        match self.kind {
            DataValueKind::U8 => {
                let byte = *byte_offset;
                *byte_offset += 1;
                quote! {
                    if let Some(byte) = bytes.get(#byte) {
                        *byte
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            DataValueKind::I8 => {
                let byte = *byte_offset;
                *byte_offset += 1;
                quote! {
                    if let Some(byte) = bytes.get(#byte) {
                        *byte as i8
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            DataValueKind::F32 => {
                let start = *byte_offset;
                *byte_offset += 4;
                quote! {
                    if let Ok(slice) = TryInto::<[u8; 4]>::try_into(&bytes[#start..#byte_offset]) {
                        f32::from_le_bytes(slice)
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            DataValueKind::F64 => {
                let start = *byte_offset;
                *byte_offset += 8;
                quote! {
                    if let Ok(slice) = TryInto::<[u8; 8]>::try_into(&bytes[#start..#byte_offset]) {
                        f64::from_le_bytes(slice)
                    } else {
                        return Err(::mini_udp::BitReprError::SliceTooShort);
                    }
                }
            }
            _ => todo!(),
        }
    }
}

impl Deref for DataValue {
    type Target = DataValueKind;
    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

#[derive(Debug, PartialEq)]
enum DataIdentifier {
    Named(syn::Ident),
    Unnamed(usize),
}

#[derive(Debug, PartialEq, Clone)]
pub enum DataValueKind {
    Bool,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    U64,
    I64,
    F64,
    USize,
    ISize,
    U128,
    I128,
    Delegated { ty: Box<syn::Type> },
}

impl DataValueKind {
    fn from_type(ty: &syn::Type) -> Self {
        match ty {
            syn::Type::Path(path) => match path.path.get_ident() {
                Some(ident) => match ident.to_string().as_str() {
                    "bool" => Self::Bool,
                    "u8" => Self::U8,
                    "i8" => Self::I8,
                    "u16" => Self::U16,
                    "i16" => Self::I16,
                    "u32" => Self::U32,
                    "i32" => Self::I32,
                    "f32" => Self::F32,
                    "u64" => Self::U64,
                    "i64" => Self::I64,
                    "f64" => Self::F64,
                    "usize" => Self::USize,
                    "isize" => Self::ISize,
                    "u128" => Self::U128,
                    "i128" => Self::I128,
                    _ => Self::Delegated {
                        ty: Box::new(ty.clone()),
                    },
                },
                None => Self::Delegated {
                    ty: Box::new(ty.clone()),
                },
            },
            _ => Self::Delegated {
                ty: Box::new(ty.clone()),
            },
        }
    }

    pub fn header_bits(&self) -> usize {
        match self {
            Self::Bool => 1,
            Self::U8 | Self::I8 => 0,
            Self::U16 | Self::I16 => 1,
            Self::U32 | Self::I32 => 1,
            Self::F32 | Self::F64 => 0,
            Self::U64 | Self::I64 => 1,
            Self::USize | Self::ISize => 1,
            Self::U128 | Self::I128 => 2,
            Self::Delegated { .. } => 0,
        }
    }

    fn bit_size(&self) -> Option<usize> {
        Some(match self {
            Self::Bool => 1,
            Self::U8 | Self::I8 => 8,
            Self::U16 | Self::I16 => 16,
            Self::U32 | Self::I32 | Self::F32 => 32,
            Self::U64 | Self::I64 | Self::F64 => 64,
            #[cfg(target_pointer_width = "32")]
            Self::USize | Self::ISize => 32,
            #[cfg(target_pointer_width = "64")]
            Self::USize | Self::ISize => 64,
            Self::U128 | Self::I128 => 128,
            Self::Delegated { .. } => return None,
        })
    }

    pub fn data_bytes(&self) -> FullDataBytes {
        match self {
            Self::Bool => FullDataBytes::None,
            Self::U8 | Self::I8 => FullDataBytes::Fixed(1),
            Self::U16 | Self::I16 => FullDataBytes::Range(1..2),
            Self::U32 | Self::I32 => FullDataBytes::Range(2..4),
            Self::F32 => FullDataBytes::Fixed(4),
            Self::U64 | Self::I64 => FullDataBytes::Range(4..8),
            Self::F64 => FullDataBytes::Fixed(8),
            #[cfg(target_pointer_width = "32")]
            Self::USize | Self::ISize => FullDataBytes::Range(2..4),
            #[cfg(target_pointer_width = "64")]
            Self::USize | Self::ISize => FullDataBytes::Range(4..8),
            Self::U128 | Self::I128 => FullDataBytes::Range(8..16),
            Self::Delegated { .. } => FullDataBytes::Delegated,
        }
    }

    fn type_name(&self) -> TokenStream2 {
        match self {
            Self::Bool => quote! {bool},
            Self::U8 => quote! { u8 },
            Self::I8 => quote! { i8 },
            Self::U16 => quote! { u16 },
            Self::I16 => quote! { i16 },
            Self::U32 => quote! { u32 },
            Self::I32 => quote! { i32 },
            Self::F32 => quote! { f32 },
            Self::U64 => quote! { u64 },
            Self::I64 => quote! { i64 },
            Self::F64 => quote! { f64 },
            Self::USize => quote! { usize },
            Self::ISize => quote! { isize },
            Self::U128 => quote! { u128 },
            Self::I128 => quote! { i128 },
            Self::Delegated { ty } => ty.to_token_stream(),
        }
    }
}

pub enum FullDataBytes {
    None,
    Fixed(usize),
    Range(Range<usize>),
    Delegated,
}

impl FullDataBytes {
    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed(_))
    }
}

pub struct Header {
    bit_len: usize,
    set_ptr: usize,
    get_ptr: usize,
}
