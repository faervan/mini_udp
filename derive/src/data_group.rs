use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};

use crate::data2::{ByteLen, Context, Field, FieldIdentifier, Value};

pub struct Fields {
    ident: TokenStream2,
    fields: Vec<Field>,
    kind: FieldKind,
    /// Stores variant name and variant id (numeric)
    enum_variant: Option<(syn::Ident, syn::LitInt)>,
}

enum FieldKind {
    Unit,
    Named,
    Unnamed,
}

impl Fields {
    pub fn new(
        ident: syn::Ident,
        fields: &syn::Fields,
        enum_variant: Option<(syn::Ident, syn::LitInt)>,
        context: &mut Context,
    ) -> Self {
        let empty = syn::punctuated::Punctuated::<syn::Field, syn::token::Comma>::new();
        let (fields, kind) = match fields {
            syn::Fields::Unit => (empty.iter(), FieldKind::Unit),
            syn::Fields::Named(fields) => (fields.named.iter(), FieldKind::Named),
            syn::Fields::Unnamed(fields) => (fields.unnamed.iter(), FieldKind::Unnamed),
        };
        let fields = fields
            .enumerate()
            .map(|(index, field)| {
                Field::new(
                    context,
                    field
                        .ident
                        .clone()
                        .map_or_else(|| FieldIdentifier::Unnamed(index), FieldIdentifier::Named),
                    Value::from_type(&field.ty),
                )
            })
            .collect();
        let ident = match &enum_variant {
            Some((variant, _)) => {
                quote! {#ident::#variant}
            }
            None => ident.into_token_stream(),
        };
        Self {
            ident,
            fields,
            kind,
            enum_variant,
        }
    }

    pub fn read(&self) -> TokenStream2 {
        let read_fixed: TokenStream2 = self.fields.iter().map(Field::read_fixed_part).collect();
        let read_variable: TokenStream2 =
            self.fields.iter().map(Field::read_variable_part).collect();
        let fields = self.bind_fields();
        let ident = &self.ident;
        match self.kind {
            FieldKind::Unit => ident.into_token_stream(),
            FieldKind::Named => quote! {
                #read_fixed
                #read_variable
                #ident { #fields }
            },
            FieldKind::Unnamed => quote! {
                #read_fixed
                #read_variable
                #ident ( #fields )
            },
        }
    }

    pub fn write(&self) -> TokenStream2 {
        let write_fixed: TokenStream2 = self.fields.iter().map(Field::write_fixed_part).collect();
        let write_variable: TokenStream2 =
            self.fields.iter().map(Field::write_variable_part).collect();
        let binding = self.binding();
        match &self.enum_variant {
            Some((_, id)) => quote! {
                #binding => {
                    #write_fixed
                    #write_variable
                    #id
                }
            },
            None => quote! {{
                let #binding = self;
                #write_fixed
                #write_variable
            }},
        }
    }

    pub fn length(&self) -> ByteLen {
        todo!()
    }

    pub fn min_variable_byte_len(&self) -> TokenStream2 {
        self.fields
            .iter()
            .map(|field| field.value.min_variable_byte_len())
            .collect()
    }

    pub fn max_variable_byte_len(&self) -> TokenStream2 {
        self.fields
            .iter()
            .map(|field| field.value.max_variable_byte_len())
            .collect()
    }

    pub fn variable_byte_len(&self) -> TokenStream2 {
        self.fields.iter().map(Field::variable_byte_len).collect()
    }

    pub fn binding(&self) -> TokenStream2 {
        let fields = self.bind_fields();
        let ident = &self.ident;
        match &self.kind {
            FieldKind::Unit => ident.to_token_stream(),
            FieldKind::Named => quote! {#ident { #fields }},
            FieldKind::Unnamed => quote! {#ident ( #fields )},
        }
    }

    fn bind_fields(&self) -> TokenStream2 {
        self.fields
            .iter()
            .map(|field| {
                let binding = syn::Ident::from(&field.ident);
                match &field.ident {
                    FieldIdentifier::Named(ident) => {
                        quote! {#ident: #binding,}
                    }
                    FieldIdentifier::Unnamed(_) => {
                        quote! {#binding,}
                    }
                }
            })
            .collect()
    }
}
