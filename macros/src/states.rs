use convert_case::{Case, Casing};
use proc_macro2::{Span, TokenStream};
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Field,
    Fields::Named,
    GenericParam, Generics, Ident, TypeParam, Visibility, WhereClause,
    WherePredicate,
    parse::{Parse, Parser},
};
use template_quote::quote;

use crate::suffix_ident;

pub fn derive_states(
    input: proc_macro::TokenStream,
) -> Result<TokenStream, syn::Error> {
    let DeriveInput { vis, ident, generics, data, .. } =
        DeriveInput::parse.parse(input)?;

    let Data::Enum(DataEnum { variants, .. }) = data else {
        return Err(syn::Error::new(Span::call_site(), "States must be enums"));
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

/* -------------------------------------------------------------------------- */

pub fn emit_view(
    vis: &Visibility,
    ident: &Ident,
    generics: &Generics,
    fields: &[Field],
) -> Result<TokenStream, syn::Error> {
    let view_ident = suffix_ident(&ident, "StatefulView");
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut s_generics = generics.clone();

    s_generics.params.push(GenericParam::parse.parse2(quote! { S })?);
    s_generics
        .where_clause
        .get_or_insert(WhereClause::parse.parse2(quote! {where})?)
        .predicates
        .push(
            WherePredicate::parse
                .parse2(quote! {S: engi::model::state::States})?,
        );

    let (s_impl_generics, s_ty_generics, s_where_clause) =
        s_generics.split_for_impl();

    fn get_ident(f: &Field) -> Result<&Ident, syn::Error> {
        f.ident.as_ref().ok_or(syn::Error::new_spanned(
            f,
            "#[derive(StatefulView)] supports only named fields",
        ))
    }

    Ok(quote! {
        impl #impl_generics engi::model::state::Stateable for #{&ident} #ty_generics #where_clause {
            type View<S> = #view_ident #s_ty_generics #s_where_clause;
        }

        #[derive(Clone)]
        #vis struct #view_ident #s_ty_generics #s_where_clause {
            #(for f in fields) {
                #{&f.vis} #{get_ident(&f)?}: engi::model::state::Stateful<#{&f.ty}, S>,
            }
        }

        impl #s_impl_generics engi::model::state::StatefulView<S> for #view_ident #s_ty_generics #s_where_clause
        {
            type Of = #{&ident} #ty_generics;

            fn new(states: &S::Storage<Self::Of>) -> Self {
                Self {
                    #(for f in fields) {
                        #{&f.ident}: engi::model::state::Stateful::new(
                            <S::Storage<#{&f.ty}> as engi::model::state::Storage<#{&f.ty}>>
                                ::from_fn(|i| Clone::clone(&states[i].#{&f.ident}))
                        ),
                    }
                }
            }
        }
    })
}
