use std::iter::Sum;
use std::ops::Add;

use enum_assoc::Assoc;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

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
        let this = Self {
            byte_offset: context.byte_offset,
            ident,
            value,
            is_part_of_enum,
        };
        let fixed_len = this.length().fixed_bytes;
        context.byte_offset += fixed_len;
        this
    }

    pub fn read_fixed_part(&self) -> TokenStream2 {
        let length = self.length();
        if length.fixed_bytes == 1 {
            let byte = self.byte_offset;
            let access =
                quote! {*bytes.get(#byte).ok_or(::mini_udp::ByteReprError::SliceTooShort)?};
            let value = match &self.value {
                Value::Bool => quote! {#access == 1},
                Value::U8 => access,
                Value::I8 => quote! {#access as i8},
                _ => unreachable!(),
            };
            let ident = syn::Ident::from(&self.ident);
            return quote! {let #ident = #value;};
        }
        let byte_start = self.byte_offset;
        let len = length.fixed_bytes;
        let byte_end = byte_start + len;
        let bytes = quote! {
            TryInto::<[u8; #len]>::try_into(
                bytes.get(#byte_start..#byte_end).ok_or(::mini_udp::ByteReprError::SliceTooShort)?
            )
            .map_err(|_| ::mini_udp::ByteReprError::SliceTooShort)?
        };
        if length.is_static() {
            let ident = syn::Ident::from(&self.ident);
            let ty = self.value.name();
            quote! {let #ident = <#ty>::from_le_bytes(#bytes);}
        } else if let Value::Vec { .. } = &self.value {
            let ident = self.fixed_ident();
            let ty = quote! {u32};
            quote! {let #ident = <#ty>::from_le_bytes(#bytes) as usize;}
        } else {
            quote! {}
        }
    }

    pub fn read_variable_part(&self) -> TokenStream2 {
        if self.length().is_static() {
            return quote! {};
        }
        let ident = syn::Ident::from(&self.ident);
        let fixed_ident = self.fixed_ident();
        match &self.value {
            Value::Delegated { ty } => {
                quote! {
                    let #ident = <#ty>::from_bytes(&bytes[byte_ptr..])?;
                    byte_ptr += #ident.byte_len();
                }
            }
            Value::Array { ty, length } => {
                quote! {
                    let mut #ident = (0..#length)
                        .map(|n| {
                            let item = <#ty>::from_bytes(&bytes[byte_ptr..])?;
                            byte_ptr += item.byte_len();
                            Ok(item)
                        })
                        // TODO! This heap allocation is unnecessary
                        .collect::<Result<Vec<_>, ::mini_udp::ByteReprError>>()?
                        .try_into()
                        .unwrap();
                }
            }
            Value::Vec { ty, .. } => {
                quote! {
                    let mut #ident = vec![];
                    for i in 0..#fixed_ident {
                        let item = <#ty>::from_bytes(&bytes[byte_ptr..])?;
                        byte_ptr += item.byte_len();
                        #ident.push(item);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn write_fixed_part(&self) -> TokenStream2 {
        let ident = syn::Ident::from(&self.ident);
        let length = self.length();
        if length.fixed_bytes == 1 {
            let byte = self.byte_offset;
            let access =
                quote! {*bytes.get_mut(#byte).ok_or(::mini_udp::ByteReprError::SliceTooShort)?};
            let byte_value = match &self.value {
                Value::Bool => quote! {if *#ident {1} else {0}},
                Value::U8 => quote! {*#ident},
                Value::I8 => quote! {*#ident as u8},
                _ => unreachable!(),
            };
            return quote! {#access = #byte_value;};
        }
        let byte_start = self.byte_offset;
        let len = match length.is_static() {
            true => length.fixed_bytes,
            false => 4,
        };
        let byte_end = byte_start + len;
        let access = quote! {
            bytes.get_mut(#byte_start..#byte_end).ok_or(::mini_udp::ByteReprError::SliceTooShort)?
        };
        let thing = match length.is_static() {
            true => ident.to_token_stream(),
            false => match self.value {
                Value::Vec { .. } => quote! {(#ident.len() as u32)},
                Value::Array { .. } | Value::Delegated { .. } => return quote! {},
                _ => unreachable!(),
            },
        };
        let bytes = quote! {
            #thing.to_le_bytes()
        };
        quote! {#access.copy_from_slice(&#bytes);}
    }

    pub fn write_variable_part(&self) -> TokenStream2 {
        if self.length().is_static() {
            return quote! {};
        }
        let ident = syn::Ident::from(&self.ident);
        match &self.value {
            Value::Delegated { .. } => {
                quote! {
                    #ident.write_to_bytes(&mut bytes[byte_ptr..])?;
                    byte_ptr += #ident.byte_len();
                }
            }
            Value::Array { .. } => {
                quote! {
                    for item in #ident.iter() {
                        item.write_to_bytes(&mut bytes[byte_ptr..])?;
                        byte_ptr += item.byte_len();
                    }
                }
            }
            Value::Vec { .. } => {
                quote! {
                    for item in #ident.iter() {
                        item.write_to_bytes(&mut bytes[byte_ptr..])?;
                        byte_ptr += item.byte_len();
                    }
                }
            }
            _ => unreachable!(),
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
#[func(fn length(&self, field: TokenStream2) -> ByteLen)]
pub enum Value {
    #[assoc(name = quote! {bool}, length = ByteLen::fully_static(1))]
    Bool,
    #[assoc(name = quote! {u8}, length = ByteLen::fully_static(1))]
    U8,
    #[assoc(name = quote! {i8}, length = ByteLen::fully_static(1))]
    I8,
    #[assoc(name = quote! {u16}, length = ByteLen::fully_static(2))]
    U16,
    #[assoc(name = quote! {i16}, length = ByteLen::fully_static(2))]
    I16,
    #[assoc(name = quote! {u32}, length = ByteLen::fully_static(4))]
    U32,
    #[assoc(name = quote! {i32}, length = ByteLen::fully_static(4))]
    I32,
    #[assoc(name = quote! {f32}, length = ByteLen::fully_static(4))]
    F32,
    #[assoc(name = quote! {u64}, length = ByteLen::fully_static(8))]
    U64,
    #[assoc(name = quote! {i64}, length = ByteLen::fully_static(8))]
    I64,
    #[assoc(name = quote! {f64}, length = ByteLen::fully_static(8))]
    F64,
    #[assoc(name = quote! {usize})]
    #[cfg_attr(target_pointer_width = "32", assoc(length = ByteLen::fully_static(4)))]
    #[cfg_attr(target_pointer_width = "64", assoc(length = ByteLen::fully_static(8)))]
    USize,
    #[assoc(name = quote! {isize})]
    #[cfg_attr(target_pointer_width = "32", assoc(length = ByteLen::fully_static(4)))]
    #[cfg_attr(target_pointer_width = "64", assoc(length = ByteLen::fully_static(8)))]
    ISize,
    #[assoc(name = quote! {u128}, length = ByteLen::fully_static(16))]
    U128,
    #[assoc(name = quote! {i128}, length = ByteLen::fully_static(16))]
    I128,
    #[assoc(
        name = quote! {[#_ty; #_length]}.to_token_stream(),
        length = ByteLen::fully_unknown(
            quote! {<#_ty>::MIN_BYTE_LEN * #_length},
            quote! {<#_ty>::MAX_BYTE_LEN * #_length},
            quote! {#field.iter().map(|f| f.byte_len()).sum::<usize>()},
        )
    )]
    Array { ty: Box<Self>, length: syn::Expr },
    #[assoc(
        name = quote! {Vec<#_ty>}.to_token_stream(),
        length = ByteLen::known_fixed_unknown_length(
            4,
            {
                let max_len = _ty.length(quote! {}).as_const_max_length();
                quote! {#_max_length * (#max_len)}
            },
            quote! {#field.iter().fold(0, |acc, item| {acc + item.byte_len()})}
        )
    )]
    Vec { ty: Box<Self>, max_length: usize },
    #[assoc(
        name = _ty.to_token_stream(),
        length = ByteLen::fully_unknown(
            quote! {<#_ty>::MIN_BYTE_LEN},
            quote! {<#_ty>::MAX_BYTE_LEN},
            quote! {#field.byte_len()}
        )
    )]
    Delegated { ty: Box<syn::Type> },
}

impl Value {
    pub fn from_type(ty: &syn::Type) -> Self {
        match ty {
            syn::Type::Path(path) => match path.path.get_ident() {
                Some(ident) => {
                    return match ident.to_string().as_str() {
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
                    };
                }
                None => match path.path.segments.last() {
                    Some(syn::PathSegment {
                        ident,
                        arguments:
                            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                args,
                                ..
                            }),
                    }) if ident == "Vec" => {
                        if let Some(syn::GenericArgument::Type(t)) = args.last() {
                            return Self::Vec {
                                ty: Box::new(Self::from_type(t)),
                                max_length: 1000,
                            };
                        }
                    }
                    _ => (),
                },
            },
            syn::Type::Array(syn::TypeArray { elem, len, .. }) => {
                return Self::Array {
                    ty: Box::new(Self::from_type(elem)),
                    length: len.clone(),
                };
            }
            _ => {}
        }
        Self::Delegated {
            ty: Box::new(ty.clone()),
        }
    }
}

impl ToTokens for Value {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend(self.name());
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
    /// Number of bytes that are always present
    fixed_bytes: usize,
    /// Known part of the minium length, excluding the `fixed_bytes`
    min_length: usize,
    /// Known part of the maximum length, excluding the `fixed_bytes`
    max_length: usize,
    /// Unknown part of the minium length
    pub unknown_min: TokenStream2,
    /// Unknown part of the maximum length
    pub unknown_max: TokenStream2,
    unknown_length: TokenStream2,
}

impl ByteLen {
    fn fully_static(length: usize) -> Self {
        Self {
            fixed_bytes: length,
            min_length: 0,
            max_length: 0,
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
            fixed_bytes: 0,
            min_length: 0,
            max_length: 0,
            unknown_min,
            unknown_max,
            unknown_length,
        }
    }

    fn known_fixed_unknown_length(
        fixed_bytes: usize,
        unknown_max: TokenStream2,
        unknown_length: TokenStream2,
    ) -> Self {
        Self {
            fixed_bytes,
            min_length: 0,
            max_length: 0,
            unknown_min: quote! {},
            unknown_max,
            unknown_length,
        }
    }

    fn is_static(&self) -> bool {
        self.unknown_min.is_empty() && self.unknown_max.is_empty() && self.unknown_length.is_empty()
    }

    pub fn as_const_min_length(&self) -> TokenStream2 {
        let min = self.known_min_length();
        if self.unknown_min.is_empty() {
            min.to_token_stream()
        } else {
            let unknown = &self.unknown_min;
            quote! {#min + #unknown}
        }
    }

    pub fn as_const_max_length(&self) -> TokenStream2 {
        let max = self.known_max_length();
        if self.unknown_max.is_empty() {
            max.to_token_stream()
        } else {
            let unknown = &self.unknown_max;
            quote! {#max + #unknown}
        }
    }

    pub fn as_length(&self) -> TokenStream2 {
        let fixed_bytes = self.fixed_bytes;
        if !self.unknown_length.is_empty() {
            let unknown = &self.unknown_length;
            return quote! {#fixed_bytes + #unknown};
        }
        assert_eq!(self.min_length, self.max_length);
        let mut len = self.known_min_length().to_token_stream();
        if !self.unknown_min.is_empty() {
            len.extend([quote! {+ }, self.unknown_min.clone()]);
        }
        if !self.unknown_max.is_empty() {
            len.extend([quote! {+ }, self.unknown_max.clone()]);
        }
        len
    }

    pub fn known_min_length(&self) -> usize {
        self.fixed_bytes + self.min_length
    }

    pub fn known_max_length(&self) -> usize {
        self.fixed_bytes + self.max_length
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
            fixed_bytes: self.fixed_bytes + rhs.fixed_bytes,
            min_length: self.min_length + rhs.min_length,
            max_length: self.max_length + rhs.max_length,
            unknown_min,
            unknown_max,
            unknown_length,
        }
    }
}
