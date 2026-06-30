use std::iter::Sum;
use std::ops::Add;

use enum_assoc::Assoc;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens as _, quote};

#[derive(Default)]
pub struct Context {
    pub byte_offset: usize,
}

pub struct Field {
    byte_offset: usize,
    pub ident: FieldIdentifier,
    pub value: Value,
    is_part_of_enum: bool,
}

impl Field {
    pub fn new(
        context: &mut Context,
        ident: FieldIdentifier,
        value: Value,
        is_part_of_enum: bool,
    ) -> Self {
        let fixed_len = value.fixed_byte_len();
        let this = Self {
            byte_offset: context.byte_offset,
            ident,
            value,
            is_part_of_enum,
        };
        context.byte_offset += fixed_len;
        this
    }

    pub fn read_fixed_part(&self) -> TokenStream2 {
        if self.value.fixed_byte_len() == 1 {
            let byte = self.byte_offset;
            let access = quote! {*bytes.get(#byte).ok_or(::mini_udp::BitReprError::SliceTooShort)?};
            let value = match &self.value {
                Value::Bool => quote! {#access & 1 << 7 == 1},
                Value::U8 => access,
                Value::I8 => quote! {#access as i8},
                Value::Delegated { .. } => quote! {#access as usize},
                _ => unreachable!(),
            };
            let ident = match self.value.static_len() {
                true => (&self.ident).into(),
                false => self.fixed_ident(),
            };
            return quote! {let #ident = #value;};
        }
        assert!(self.value.static_len());
        let byte_start = self.byte_offset;
        let len = self.value.fixed_byte_len();
        let byte_end = byte_start + len;
        let bytes = quote! {
            TryInto::<[u8; #len]>::try_into(&bytes[#byte_start..#byte_end])
                .map_err(|_| ::mini_udp::BitReprError::SliceTooShort)?
        };
        let ident = syn::Ident::from(&self.ident);
        let ty = self.value.name();
        quote! {let #ident = #ty::from_le_bytes(#bytes);}
    }

    pub fn read_variable_part(&self) -> TokenStream2 {
        if self.value.static_len() {
            return quote! {};
        }
        let Value::Delegated { ty } = &self.value else {
            unreachable!()
        };
        let ident = syn::Ident::from(&self.ident);
        let fixed_ident = self.fixed_ident();
        quote! {
            let #ident = #ty::from_bytes(&bytes[byte_ptr..])?;
            byte_ptr += #fixed_ident;
        }
    }

    pub fn write_fixed_part(&self) -> TokenStream2 {
        let ident = syn::Ident::from(&self.ident);
        if self.value.fixed_byte_len() == 1 {
            let byte = self.byte_offset;
            let access =
                quote! {*bytes.get_mut(#byte).ok_or(::mini_udp::BitReprError::SliceTooShort)?};
            let byte_value = match &self.value {
                Value::Bool => quote! {if *#ident {0_u8 & 1 << 7} else {0}},
                Value::U8 => quote! {*#ident},
                Value::I8 => quote! {*#ident as u8},
                Value::Delegated { .. } => quote! {#ident.bit_len() as u8},
                _ => unreachable!(),
            };
            return quote! {#access = #byte_value;};
        }
        assert!(self.value.static_len());
        let byte_start = self.byte_offset;
        let len = self.value.fixed_byte_len();
        let byte_end = byte_start + len;
        let access = quote! {
            bytes[#byte_start..#byte_end]
        };
        let bytes = quote! {
            #ident.to_le_bytes()
        };
        quote! {#access.copy_from_slice(&#bytes);}
    }

    pub fn write_variable_part(&self) -> TokenStream2 {
        if self.value.static_len() {
            return quote! {};
        }
        let ident = syn::Ident::from(&self.ident);
        quote! {
            #ident.write_to_bytes(&mut bytes[byte_ptr..])?;
            byte_ptr += #ident.bit_len();
        }
    }

    pub fn variable_byte_len(&self) -> TokenStream2 {
        match self.value {
            Value::Delegated { .. } => {
                let field_access = match &self.ident {
                    FieldIdentifier::Named(ident) => ident.to_token_stream(),
                    FieldIdentifier::Unnamed(index) => index.to_token_stream(),
                };
                quote! {+ self.#field_access.bit_len()}
            }
            _ => quote! {},
        }
    }

    pub fn length(&self) -> ByteLen {
        let field = match self.is_part_of_enum {
            true => syn::Ident::from(&self.ident).into_token_stream(),
            false => {
                let field = match &self.ident {
                    FieldIdentifier::Named(ident) => ident.to_token_stream(),
                    FieldIdentifier::Unnamed(index) => syn::Index::from(*index).into_token_stream(),
                };
                quote! {self.#field}
            }
        };
        self.value.length(field)
    }

    fn fixed_ident(&self) -> syn::Ident {
        syn::Ident::new(
            &format!("{}_fixed", syn::Ident::from(&self.ident)),
            Span::mixed_site(),
        )
    }
}

#[derive(Debug, PartialEq, Clone, Assoc)]
#[func(fn name(&self) -> TokenStream2)]
#[func(fn static_len(&self) -> bool {true})]
#[func(fn fixed_byte_len(&self) -> usize {1})]
#[func(pub fn min_variable_byte_len(&self) -> TokenStream2 {quote! {}})]
#[func(pub fn max_variable_byte_len(&self) -> TokenStream2 {quote! {}})]
#[func(fn length(&self, field: TokenStream2) -> ByteLen)]
pub enum Value {
    #[assoc(name = quote! {bool}, length = ByteLen::fully_static(1))]
    Bool,
    #[assoc(name = quote! {u8}, length = ByteLen::fully_static(1))]
    U8,
    #[assoc(name = quote! {i8}, length = ByteLen::fully_static(1))]
    I8,
    #[assoc(name = quote! {u16}, fixed_byte_len = 2, length = ByteLen::fully_static(2))]
    U16,
    #[assoc(name = quote! {i16}, fixed_byte_len = 2, length = ByteLen::fully_static(2))]
    I16,
    #[assoc(name = quote! {u32}, fixed_byte_len = 4, length = ByteLen::fully_static(4))]
    U32,
    #[assoc(name = quote! {i32}, fixed_byte_len = 4, length = ByteLen::fully_static(4))]
    I32,
    #[assoc(name = quote! {f32}, fixed_byte_len = 4, length = ByteLen::fully_static(4))]
    F32,
    #[assoc(name = quote! {u64}, fixed_byte_len = 8, length = ByteLen::fully_static(8))]
    U64,
    #[assoc(name = quote! {i64}, fixed_byte_len = 8, length = ByteLen::fully_static(8))]
    I64,
    #[assoc(name = quote! {f64}, fixed_byte_len = 8, length = ByteLen::fully_static(8))]
    F64,
    #[assoc(name = quote! {usize})]
    #[cfg_attr(target_pointer_width = "32", assoc(fixed_byte_len = 4, length = ByteLen::fully_static(4)))]
    #[cfg_attr(target_pointer_width = "64", assoc(fixed_byte_len = 8, length = ByteLen::fully_static(8)))]
    USize,
    #[assoc(name = quote! {isize})]
    #[cfg_attr(target_pointer_width = "32", assoc(fixed_byte_len = 4, length = ByteLen::fully_static(4)))]
    #[cfg_attr(target_pointer_width = "64", assoc(fixed_byte_len = 8, length = ByteLen::fully_static(8)))]
    ISize,
    #[assoc(name = quote! {u128}, fixed_byte_len = 16, length = ByteLen::fully_static(16))]
    U128,
    #[assoc(name = quote! {i128}, fixed_byte_len = 16, length = ByteLen::fully_static(16))]
    I128,
    #[assoc(
        name = _ty.to_token_stream(),
        static_len = false,
        min_variable_byte_len = quote! {+ #_ty::MIN_BIT_LEN},
        max_variable_byte_len = quote! {+ #_ty::MAX_BIT_LEN},
        length = ByteLen::fully_unknown(
            quote! {#_ty::MIN_BIT_LEN},
            quote! {#_ty::MAX_BIT_LEN},
            quote! {#field.bit_len()}
        )
    )]
    Delegated { ty: Box<syn::Type> },
}

impl Value {
    pub fn from_type(ty: &syn::Type) -> Self {
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
}

#[derive(Debug, PartialEq)]
pub enum FieldIdentifier {
    Named(syn::Ident),
    Unnamed(usize),
}

impl From<&FieldIdentifier> for syn::Ident {
    fn from(value: &FieldIdentifier) -> Self {
        match value {
            FieldIdentifier::Named(ident) => {
                syn::Ident::new(&format!("named_{ident}"), Span::mixed_site())
            }
            FieldIdentifier::Unnamed(index) => {
                syn::Ident::new(&format!("unnamed_{index}"), Span::mixed_site())
            }
        }
    }
}

pub struct ByteLen {
    pub min_length: usize,
    pub max_length: usize,
    pub unknown_min: TokenStream2,
    pub unknown_max: TokenStream2,
    unknown_length: TokenStream2,
}

impl ByteLen {
    fn fully_static(length: usize) -> Self {
        Self {
            min_length: length,
            max_length: length,
            unknown_min: quote! {},
            unknown_max: quote! {},
            unknown_length: quote! {},
        }
    }

    fn known_variable(min: usize, max: usize) -> Self {
        Self {
            min_length: min,
            max_length: max,
            unknown_min: quote! {},
            unknown_max: quote! {},
            unknown_length: quote! {},
        }
    }

    fn fully_unknown(
        unknown_min: TokenStream2,
        unknown_max: TokenStream2,
        unknown_length: TokenStream2,
    ) -> Self {
        Self {
            min_length: 0,
            max_length: 0,
            unknown_min,
            unknown_max,
            unknown_length,
        }
    }

    pub fn as_const_min_length(&self) -> TokenStream2 {
        let min = self.min_length;
        if self.unknown_min.is_empty() {
            min.to_token_stream()
        } else {
            let unknown = &self.unknown_min;
            quote! {#min + #unknown}
        }
    }

    pub fn as_const_max_length(&self) -> TokenStream2 {
        let max = self.max_length;
        if self.unknown_max.is_empty() {
            max.to_token_stream()
        } else {
            let unknown = &self.unknown_max;
            quote! {#max + #unknown}
        }
    }

    pub fn as_length(&self) -> TokenStream2 {
        if !self.unknown_length.is_empty() {
            return self.unknown_length.clone();
        }
        assert_eq!(self.min_length, self.max_length);
        let mut len = self.min_length.to_token_stream();
        if !self.unknown_min.is_empty() {
            len.extend([quote! {+ }, self.unknown_min.clone()]);
        }
        if !self.unknown_max.is_empty() {
            len.extend([quote! {+ }, self.unknown_max.clone()]);
        }
        len
    }
}

impl Sum for ByteLen {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(ByteLen::fully_static(0), |acc, next| acc + next)
    }
}

impl Add<Self> for ByteLen {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let min_a = self.unknown_min;
        let min_b = rhs.unknown_min;
        let max_a = self.unknown_max;
        let max_b = rhs.unknown_max;
        let len_a = self.unknown_length;
        let len_b = rhs.unknown_length;

        let unknown_min = if !min_a.is_empty() && !min_b.is_empty() {
            quote! {#min_a + #min_b}
        } else {
            quote! {#min_a #min_b}
        };
        let unknown_max = if !max_a.is_empty() && !max_b.is_empty() {
            quote! {#max_a + #max_b}
        } else {
            quote! {#max_a #max_b}
        };
        let unknown_length = if !len_a.is_empty() && !len_b.is_empty() {
            quote! {#len_a + #len_b}
        } else {
            quote! {#len_a #len_b}
        };
        Self {
            min_length: self.min_length + rhs.min_length,
            max_length: self.max_length + rhs.max_length,
            unknown_min,
            unknown_max,
            unknown_length,
        }
    }
}
