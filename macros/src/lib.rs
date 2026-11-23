#![feature(proc_macro_totokens)]
#![feature(try_blocks)]

use std::{collections::HashMap, iter::once};

use convert_case::{Case, Casing};
use itertools::Itertools;
use proc_macro::{TokenStream, TokenTree};
use proc_macro2::{Spacing, Span};
// use quote::{ToTokens, format_ident, quote};
use syn::{
    BinOp, Data, DataStruct, DeriveInput, Expr, ExprAssign, ExprBinary,
    ExprTuple, Field, GenericArgument, Ident, LitStr, PathArguments, Token,
    Type,
    parse::{Parse, ParseBuffer, ParseStream, Parser},
    parse_macro_input,
    punctuated::Punctuated,
};
use template_quote::quote;

use crate::{
    interface::derive_interface, model::derive_model, states::derive_states,
};

/* -------------------------------------------------------------------------- */

mod interface;
mod model;
mod relation;
mod states;

/* -------------------------------------------------------------------------- */

fn get_stateful(f: &Field) -> Result<Option<(Type, Field)>, syn::Error> {
    Ok(
        if let Type::Path(ref p) = f.ty
            && let Some(segment) = p.path.segments.last()
            && segment.ident == "Stateful"
        {
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return Err(syn::Error::new_spanned(
                    p,
                    "Expected generic arguments to Stateful<T, S>",
                ));
            };

            if args.args.len() != 2 {
                return Err(syn::Error::new_spanned(
                    p,
                    "Expected exactly two generic arguments to Stateful<T, S>",
                ));
            }

            let mut iter = args.args.iter();

            let GenericArgument::Type(t) = iter.next().unwrap() else {
                return Err(syn::Error::new_spanned(
                    p,
                    "First argument must be a type",
                ));
            };

            let GenericArgument::Type(s) = iter.next().unwrap() else {
                return Err(syn::Error::new_spanned(
                    p,
                    "Second argument must be a type",
                ));
            };

            // this field if it was of type T instead of Stateful<T, S>
            let inner_field = Field { ty: t.clone(), ..f.clone() };
            Some((s.clone(), inner_field))
        } else {
            None
        },
    )
}

fn suffix_ident(ident: &Ident, suffix: &str) -> Ident {
    Ident::new(&format!("{ident}{suffix}"), Span::call_site())
}

/* -------------------------------------------------------------------------- */

#[proc_macro_derive(Model, attributes(var, model, interface))]
pub fn model(input: TokenStream) -> TokenStream {
    match derive_model(input.into()) {
        Ok(out) => out,
        Err(err) => err.into_compile_error().into(),
    }
}

/* -------------------------------------------------------------------------- */

#[proc_macro_derive(Interface, attributes(connect, conn))]
pub fn interface(input: TokenStream) -> TokenStream {
    match derive_interface(input.into()) {
        Ok(out) => out,
        Err(err) => err.into_compile_error().into(),
    }
}

/* -------------------------------------------------------------------------- */

#[proc_macro]
pub fn relations(input: TokenStream) -> TokenStream {
    relation::relations(input)
}

#[proc_macro]
pub fn relation(input: TokenStream) -> TokenStream {
    relation::relation(input)
}

/* -------------------------------------------------------------------------- */

#[proc_macro_derive(States)]
pub fn states(input: TokenStream) -> TokenStream {
    match derive_states(input) {
        Ok(out) => out.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
