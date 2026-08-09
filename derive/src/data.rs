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
    /// If `true`, use a variable identifier to access this field when calculating the length.
    /// If `false`, use `self.field` to access this field when calculating the length.
    use_variable_field_access: bool,
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
            use_variable_field_access: is_part_of_enum,
        };
        let fixed_len = this.length().fixed_bytes;
        context.byte_offset += fixed_len;
        this
    }

    fn copy_with(&self, ident: FieldIdentifier, value: Value) -> Self {
        Self {
            byte_offset: self.byte_offset,
            ident,
            value,
            use_variable_field_access: true,
        }
    }

    pub fn read_fixed_part(&self) -> TokenStream2 {
        let length = self.length();
        let ident = self.ident.as_tokens(true);
        match self.value {
            Value::EmptyTuple => return quote! {let #ident = ();},
            Value::Cow { .. } => return quote! {},
            _ => {}
        }
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
            let ty = self.value.name();
            quote! {let #ident = <#ty>::from_le_bytes(#bytes);}
        } else if let Value::String { .. } | Value::Vec { .. } = &self.value {
            let ident = self.fixed_ident();
            let ty = quote! {u32};
            quote! {let #ident = <#ty>::from_le_bytes(#bytes) as usize;}
        } else {
            quote! {}
        }
    }

    pub fn read_variable_part(&self) -> TokenStream2 {
        if self.length().is_static() && !matches!(self.value, Value::Cow { .. }) {
            return quote! {};
        }
        let ident = self.ident.as_tokens(true);
        let fixed_ident = self.fixed_ident();
        match &self.value {
            Value::Delegated { ty } => {
                quote! {
                    let #ident = <#ty>::from_bytes(&bytes[byte_ptr..])?;
                    byte_ptr += #ident.byte_len();
                }
            }
            Value::String { .. } => {
                quote! {
                    let #ident = String::from_utf8_lossy(
                        &bytes[byte_ptr..byte_ptr + #fixed_ident]
                    ).into_owned();
                }
            }
            Value::Cow { ty } => {
                let inner_identifier = FieldIdentifier::from_name("inner");
                let inner_ident = inner_identifier.as_tokens(true);
                let inner = self.copy_with(inner_identifier, (**ty).clone());
                let fixed_inner = inner.read_fixed_part();
                let variable_inner = inner.read_variable_part();
                let length = inner.length().as_length();
                quote! {
                    let #ident = {
                        #fixed_inner
                        #variable_inner
                        byte_ptr += #length;
                        std::borrow::Cow::Owned(#inner_ident)
                    };
                }
            }
            Value::Array { ty, length } => {
                quote! {
                    let mut err = Ok(());
                    let #ident: [#ty; #length] = std::array::from_fn(|_| {
                        if err.is_err() {
                            // Default will not be necessary if
                            // <https://doc.rust-lang.org/std/array/fn.try_from_fn.html> hits stable
                            return #ty::default();
                        }
                        match <#ty>::from_bytes(&bytes[byte_ptr..]) {
                            Ok(item) => {
                                byte_ptr += item.byte_len();
                                item
                            }
                            Err(e) => {
                                err = Err(e);
                                #ty::default()
                            }
                        }
                    });
                    let _ = err?;
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
        if let Value::EmptyTuple = self.value {
            return quote! {};
        }
        let ident = self.ident.as_tokens(false);
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
            true => ident,
            false => match self.value {
                Value::String { .. } | Value::Vec { .. } => quote! {(#ident.len() as u32)},
                Value::Array { .. } | Value::Cow { .. } | Value::Delegated { .. } => {
                    return quote! {};
                }
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
        let ident = self.ident.as_tokens(false);
        match &self.value {
            Value::Delegated { .. } => {
                quote! {
                    #ident.write_to_bytes(&mut bytes[byte_ptr..])?;
                    byte_ptr += #ident.byte_len();
                }
            }
            Value::String { .. } => {
                quote! {
                    bytes.get_mut(byte_ptr..byte_ptr + #ident.len())
                        .ok_or(::mini_udp::ByteReprError::SliceTooShort)?
                        .copy_from_slice(#ident.as_bytes());
                    byte_ptr += #ident.len();
                }
            }
            Value::Cow { ty } => {
                let inner_identifier =
                    FieldIdentifier::Access(quote! {std::borrow::Borrow::<#ty>::borrow(#ident)});
                let inner = self.copy_with(inner_identifier, (**ty).clone());
                let fixed_inner = inner.write_fixed_part();
                let variable_inner = inner.write_variable_part();
                let length = inner.length().as_length();
                quote! {
                    #fixed_inner
                    #variable_inner
                    byte_ptr += #length;
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
        let field = match self.use_variable_field_access {
            true => self.ident.as_tokens(false),
            false => {
                let field = match &self.ident {
                    FieldIdentifier::Named(ident) => ident.to_token_stream(),
                    FieldIdentifier::Unnamed(index) => syn::Index::from(*index).into_token_stream(),
                    FieldIdentifier::Access(_) => unreachable!(),
                    FieldIdentifier::None => quote! {self},
                };
                quote! {self.#field}
            }
        };
        self.value.length(field)
    }

    fn fixed_ident(&self) -> syn::Ident {
        syn::Ident::new(
            &format!("{}_fixed", self.ident.as_tokens(true)),
            Span::mixed_site(),
        )
    }
}

#[derive(Debug, PartialEq, Clone, Assoc)]
#[func(fn name(&self) -> TokenStream2)]
#[func(fn length(&self, field: TokenStream2) -> ByteLen)]
pub enum Value {
    #[assoc(name = quote! {()}, length = ByteLen::fully_static(0))]
    EmptyTuple,
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
        name = match _is_str {
            true => quote! {str},
            false => quote! {String},
        },
        length = ByteLen::known_fixed_unknown_length(
            4,
            quote! {#_max_length},
            quote! {#field.len()}
        )
    )]
    String { max_length: usize, is_str: bool },
    #[assoc(
        name = quote! {Cow<#_ty>}.to_token_stream(),
        length = _ty.length(field),
    )]
    Cow { ty: Box<Self> },
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
                        "String" => Self::String {
                            max_length: 1020,
                            is_str: false,
                        },
                        "str" => Self::String {
                            max_length: 1020,
                            is_str: true,
                        },
                        _ => Self::Delegated {
                            ty: Box::new(ty.clone()),
                        },
                    };
                }
                None => {
                    if let Some(syn::PathSegment {
                        ident,
                        arguments:
                            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                args,
                                ..
                            }),
                    }) = path.path.segments.last()
                    {
                        match ident.to_string().as_str() {
                            "Vec" => {
                                if let Some(syn::GenericArgument::Type(t)) = args.last() {
                                    return Self::Vec {
                                        ty: Box::new(Self::from_type(t)),
                                        max_length: 1000,
                                    };
                                }
                            }
                            "Cow" => {
                                if let Some(syn::GenericArgument::Type(t)) = args.last() {
                                    return Self::Cow {
                                        ty: Box::new(Self::from_type(t)),
                                    };
                                }
                            }
                            _ => {}
                        }
                    }
                }
            },
            syn::Type::Tuple(tuple) if tuple.elems.is_empty() => return Self::EmptyTuple,
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

#[derive(Debug, Clone)]
pub enum FieldIdentifier {
    Named(syn::Ident),
    Unnamed(usize),
    Access(TokenStream2),
    None,
}

impl FieldIdentifier {
    fn from_name(name: &str) -> Self {
        Self::Named(syn::Ident::new(name, Span::mixed_site()))
    }

    pub fn as_tokens(&self, is_binding: bool) -> TokenStream2 {
        match self {
            FieldIdentifier::Named(ident) => {
                syn::Ident::new(&format!("named_{ident}"), Span::mixed_site()).to_token_stream()
            }
            FieldIdentifier::Unnamed(index) => {
                syn::Ident::new(&format!("unnamed_{index}"), Span::mixed_site()).to_token_stream()
            }
            FieldIdentifier::Access(access) => access.clone(),
            FieldIdentifier::None if is_binding => quote! {this},
            FieldIdentifier::None => quote! {self},
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

    pub fn is_static(&self) -> bool {
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
