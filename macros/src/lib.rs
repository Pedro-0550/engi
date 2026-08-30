use std::{collections::HashMap, iter::once};

use darling::{FromDeriveInput, FromField, FromMeta, ast, util};
use itertools::Itertools;
use proc_macro::{TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{
    Data, DataStruct, DeriveInput, Expr, ExprTuple, Field, Ident, LitStr,
    Token, Type,
    parse::{Parse, ParseBuffer, ParseStream, Parser},
    parse_macro_input,
    punctuated::Punctuated,
};

#[derive(Debug, FromMeta)]
struct VariableAttr {
    unit: Option<Expr>,
    desc: Option<LitStr>,
    shape: Option<Expr>,
}

// #[derive(FromMeta)]
// struct SystemAttr {
//     unit:
// }

#[proc_macro_derive(Model, attributes(var, model, interface))]
pub fn model(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let DeriveInput { attrs, vis, ident, generics, data } =
        parse_macro_input!(input as DeriveInput);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let Data::Struct(DataStruct { fields, .. }) = data else {
        panic!("#[derive(System)] only supports enums");
    };

    let (mut vars, mut submodels, mut interfaces) =
        (Vec::new(), Vec::new(), Vec::new());

    for field in &fields {
        let mut attrs = HashMap::new();

        for ident in ["var", "interface", "model"] {
            let Ok(attr) = field
                .attrs
                .iter()
                .filter(|x| {
                    x.path().is_ident(&Ident::from_string(ident).unwrap())
                })
                .at_most_one()
            else {
                let message = syn::LitStr::from_string(&format!(
                    "A field cannot have more than one #[{ident}] attr"
                ))
                .unwrap();
                return quote! {compile_error!(#message);}.into();
            };

            if let Some(attr) = attr {
                attrs.insert(ident, attr);
            }
        }

        if attrs.len() > 1 {
            return quote! {compile_error!("#[var], #[interface], and #[model] are mutually exclusive.");}.into();
        }

        match attrs.into_iter().next() {
            Some(("var", var_attr)) => {
                let mut var = VariableAttr::from_meta(&var_attr.meta).unwrap();

                let desc = var.desc.unwrap_or(LitStr::from_string("").unwrap());
                let unit = var.unit.unwrap_or(
                    Expr::parse
                        .parse(quote! {engi::dimension::Unit::Unitless}.into())
                        .unwrap(),
                );

                let shape = var.shape.unwrap_or(
                    Expr::parse
                        .parse(quote! {engi::expr::Shape::SCALAR}.into())
                        .unwrap(),
                );

                vars.push((field, desc, unit, shape));
            }
            Some(("interface", interface_attr)) => {
                interfaces.push(field);
            }
            Some(("model", model_attr)) => {
                submodels.push(field);
            }
            _ => (),
        }
    }

    let variable_terms = vars.iter().map(|(field, unit, desc, shape)| {
        let name = LitStr::from_string(&field.ident.unwrap().to_string()).unwrap();
        quote! {
            engi::system::Variable::new(engi::symbol::Symbol::new(#name).set_unit(#unit).set_desc(#desc).set_shape(#shape))
        }
    });

    let solution_ident = format_ident!("{}Solution", ident);
    let builder_ident = format_ident!("{}Builder", ident);

    quote! {
        #vis struct #builder_ident {
            system: engi::system::SystemInnerRef,
            #(#builder_fields),*
        }

        #vis struct #solution_ident {
            #(#solution_fields),*
        }

        impl #impl_generics Model for #ident #ty_generics #where_clause {
            type Solution = #solution_ident;
            type Builder = #builder_ident;

            fn submodels(&self) -> Vec<Box<dyn ErasedModel>> {
                vec![
                    #(self.#submodel_idents.erased()),*
                ]
            }

            fn variables(&self) -> Vec<Variable> {
                vec![
                    #(self.#variable_idents),*
                ]
            }

            fn interface(&self) -> Vec<Box<dyn Interface>> {
                vec![
                    #(self.#interface_idents.erased()),*
                ]
            }
        }
    }
}

/* -------------------------------------------------------------------------- */

enum Constraint {
    Eq,
    Gt,
    Ge,
    Lt,
    Le,
}

struct Equation {
    lhs: Expr,
    constraint: Constraint,
    rhs: Expr,
}

impl Parse for Equation {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let lhs: Expr = input.parse()?;

        let constraint = if input.peek(Token![>=]) {
            input.parse::<Token![>=]>()?;
            Constraint::Ge
        } else if input.peek(Token![<=]) {
            input.parse::<Token![<=]>()?;
            Constraint::Le
        } else if input.peek(Token![>]) {
            input.parse::<Token![>]>()?;
            Constraint::Gt
        } else if input.peek(Token![<]) {
            input.parse::<Token![<]>()?;
            Constraint::Lt
        } else if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Constraint::Eq
        } else {
            return Err(input.error("expected =, >, >=, <, or <="));
        };

        let rhs: Expr = input.parse()?;

        Ok(Self { lhs, constraint, rhs })
    }
}

#[proc_macro]
pub fn equations(input: TokenStream) -> TokenStream {
    let punc = Punctuated::<Equation, Token![;]>::parse_terminated
        .parse(input)
        .unwrap();

    let terms =
        punc.iter().map(|Equation { lhs, constraint, rhs }| match constraint {
            Constraint::Eq => quote! {
                engi::system::eq::Equation::new(lhs, rhs)
            },
            Constraint::Gt => quote! {
                engi::system::eq::Constraint::new(#lhs, #rhs, engi::system::eq::Inequality::Greater)
            },
            Constraint::Ge => quote! {
                engi::system::eq::Constraint::new(#lhs, #rhs, engi::system::eq::Inequality::GreaterOrEq)
            },
            Constraint::Lt => quote! {
                engi::system::eq::Constraint::new(#lhs, #rhs, engi::system::eq::Inequality::Less)
            },
            Constraint::Le => quote! {
                engi::system::eq::Constraint::new(#lhs, #rhs, engi::system::eq::Inequality::LessOrEq)
            },
        });

    quote! {vec![#(#terms),*]}.into()
}
