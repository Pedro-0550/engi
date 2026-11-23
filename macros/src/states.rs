use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use syn::{
    Data, DataEnum, DeriveInput,
    parse::{Parse, Parser},
};
use template_quote::quote;

pub fn derive_states(
    input: proc_macro::TokenStream,
) -> Result<TokenStream, syn::Error> {
    let DeriveInput { vis, ident, generics, data, .. } =
        DeriveInput::parse.parse(input)?;

    let Data::Enum(DataEnum { variants, .. }) = data else {
        return Err(syn::Error::new_spanned(generics, "States must be enums"));
    };

    if generics.params.len() > 0 {
        return Err(syn::Error::new_spanned(
            generics,
            "States cannot have generic parameters",
        ));
    }

    for v in &variants {
        if !v.fields.is_empty() {
            return Err(syn::Error::new_spanned(
                &v.fields,
                "State variants cannot have fields",
            ));
        }
    }

    Ok(quote! {
        impl engi::model::state::States for #ident {
            const VARIANTS: &'static [Self] = &[
                #(for v in &variants) {
                    Self::#{&v.ident},
                }
            ];
            type Storage<T: Clone> = [T; #{ variants.len() }];

            fn idx(&self) -> usize {
                match self {
                    #(for (i, v) in variants.iter().enumerate()) {
                        Self::#{&v.ident} => #{i},
                    }
                }
            }

            fn name(&self) -> &'static str {
                match self {
                    #(for v in &variants) {
                        Self::#{&v.ident} => #{&v.ident.to_string().to_case(Case::Snake)},
                    }
                }
            }
        }
    })
}
