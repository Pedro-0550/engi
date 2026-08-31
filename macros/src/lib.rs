use std::{collections::HashMap, iter::once};

use convert_case::{Case, Casing};
use itertools::Itertools;
use proc_macro::{TokenStream, TokenTree};
use proc_macro2::{Spacing, Span};
use quote::{ToTokens, format_ident, quote};
use syn::{
    BinOp, Data, DataStruct, DeriveInput, Expr, ExprAssign, ExprBinary,
    ExprTuple, Field, Ident, LitStr, Token, Type,
    parse::{Parse, ParseBuffer, ParseStream, Parser},
    parse_macro_input,
    punctuated::Punctuated,
};

struct VariableAttr {
    unit: Option<Expr>,
    desc: Option<LitStr>,
    shape: Option<Expr>,
}

impl VariableAttr {
    fn parse(attr: &syn::Attribute) -> syn::Result<Self> {
        let mut unit = None;
        let mut desc = None;
        let mut shape = None;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("unit") {
                unit = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("desc") {
                desc = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("shape") {
                shape = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("unknown #[var] argument"));
            }

            Ok(())
        })?;

        Ok(Self { unit, desc, shape })
    }
}

// #[derive(FromMeta)]
// struct SystemAttr {
//     unit:
// }

#[proc_macro_derive(Model, attributes(var, model, interface))]
pub fn model(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let DeriveInput { vis, ident, generics, data, .. } =
        parse_macro_input!(input as DeriveInput);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let Data::Struct(DataStruct { fields, .. }) = data else {
        panic!("#[derive(Model)] only supports enums");
    };

    let (mut vars, mut submodels, mut interfaces) =
        (Vec::new(), Vec::new(), Vec::new());

    for field in &fields {
        let mut attrs = HashMap::new();

        for ident in ["var", "interface", "model"] {
            let Ok(attr) = field
                .attrs
                .iter()
                .filter(|x| x.path().is_ident(ident))
                .at_most_one()
            else {
                let message = syn::LitStr::new(
                    &format!(
                        "A field cannot have more than one #[{ident}] attr"
                    ),
                    Span::call_site(),
                );
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
                let var = match VariableAttr::parse(var_attr) {
                    Ok(var) => var,
                    Err(e) => return e.into_compile_error().into(),
                };

                let desc =
                    var.desc.unwrap_or(LitStr::new("", Span::call_site()));
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

    let solution_ident = format_ident!("{}Solution", ident);
    let builder_ident = format_ident!("{}Builder", ident);
    let variable_idents = vars.iter().map(|f| f.0.ident.clone().unwrap());
    let variable_idents2 = variable_idents.clone();

    let submodel_idents = submodels.iter().map(|f| f.ident.clone().unwrap());
    let submodel_idents2 = submodel_idents.clone();

    let interface_idents = interfaces.iter().map(|f| f.ident.clone().unwrap());
    let interface_idents2 = interface_idents.clone();

    let variable_builder_idents = vars.iter().map(|(field, ..)| {
        format_ident!("{}_builder", field.ident.clone().unwrap())
    });
    let variable_builder_idents2 = variable_builder_idents.clone();

    let submodel_builder_idents = submodels
        .iter()
        .map(|field| format_ident!("{}_builder", field.ident.clone().unwrap()));
    let submodel_builder_idents2 = submodel_builder_idents.clone();

    let interface_builder_idents = interfaces
        .iter()
        .map(|field| format_ident!("{}_builder", field.ident.clone().unwrap()));
    let interface_builder_idents2 = interface_builder_idents.clone();

    let builder_fields = vars
        .iter()
        .map(|(Field { vis, ident, .. }, ..)| {
            quote! {
                #vis #ident: engi::system::VariableBuilder
            }
        })
        .chain(submodels.iter().map(|Field { vis, ident, ty, .. }| {
            quote! {
                #vis #ident: <#ty as engi::system::Model>::Builder
            }
        }))
        .chain(interfaces.iter().map(|Field { vis, ident, .. }| {
            quote! {
                #vis #ident: engi::system::InterfaceBuilder
            }
        }));

    let default_constructor = vars
        .iter()
        .map(|(field, desc, unit, shape)| {
            let field_ident = field.ident.clone().unwrap();
            let sym_name = LitStr::new(
                field_ident.to_string().as_str(),
                Span::call_site(),
            );
            quote! {
                #field_ident: engi::system::Variable::new(
                    engi::symbol::Symbol::new(#sym_name)
                        .set_unit(#unit)
                        .set_shape(#shape)
                        .set_desc(#desc.to_owned())
                )
            }
        })
        .chain(interfaces.iter().map(|f| {
            let field_ident = f.ident.clone().unwrap();
            quote! {
                #field_ident: Default::default()
            }
        }))
        .chain(submodels.iter().map(|f| {
            let field_ident = f.ident.clone().unwrap();
            quote! {
                #field_ident: Default::default()
            }
        }));

    let solution_fields = vars
        .iter()
        .map(|(Field { vis, ident, .. }, ..)| {
            quote! {
                #vis #ident: engi::system::Value
            }
        })
        .chain(submodels.iter().map(|Field { vis, ident, ty, .. }| {
            quote! {
                #vis #ident: <#ty as engi::system::Model>::Solution
            }
        }));

    quote! {
        #vis struct #builder_ident {
            __system: engi::system::System,
            __id: engi::system::ModelId,
            #(#builder_fields,)*
        }

        impl engi::system::ModelBuilder for #builder_ident {}

        #vis struct #solution_ident {
            #(#solution_fields,)*
        }

        impl #impl_generics Default for #ident #ty_generics #where_clause {
            fn default() -> #ident {
                #ident {
                    #(#default_constructor,)*
                }
            }
        }

        impl #impl_generics engi::system::Model for #ident #ty_generics #where_clause {
            type Solution = #solution_ident;
            type Builder = #builder_ident;

            fn register(self, system: System) -> Self::Builder {

                #(let #variable_builder_idents = {
                    let id = system.0.borrow_mut().add_variable(self.#variable_idents);
                    engi::system::VariableBuilder::new(system.clone(), id)
                };)*

                #(let #interface_builder_idents = {
                    let id = system.0.borrow_mut().add_interface(self.#interface_idents);
                    engi::system::InterfaceBuilder::new(system.clone(), id)
                };)*

                #(let #submodel_builder_idents = self.#submodel_idents.register(system.clone());)*

                let own_id = system.0.borrow_mut().add_model(self);

                #builder_ident {
                    __system: system,
                    __id: own_id,
                    #(#submodel_idents2: #submodel_builder_idents2,)*
                    #(#interface_idents2: #interface_builder_idents2,)*
                    #(#variable_idents2: #variable_builder_idents2,)*
                }
            }
        }
    }.into()
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
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut lhs = TokenStream::new();
        let mut constraint = None;
        let mut rhs = TokenStream::new();

        input
            .step(|cursor| {
                let mut rest = *cursor;

                while let Some((tt, mut next)) = rest.token_tree() {
                    if let proc_macro2::TokenTree::Punct(p) = &tt
                        && p.as_char() == ';'
                    {
                        break;
                    }

                    if constraint.is_some() {
                        rhs.extend(proc_macro::TokenStream::from(
                            tt.to_token_stream(),
                        ));
                    } else if let proc_macro2::TokenTree::Punct(punct) = &tt
                        && matches!(punct.as_char(), '>' | '<' | '=')
                    {
                        let token = if punct.spacing() == Spacing::Joint {
                            if let Some((
                                proc_macro2::TokenTree::Punct(next_punct),
                                next_cursor,
                            )) = next.token_tree()
                            {
                                next = next_cursor;

                                let mut token = punct.as_char().to_string();
                                token.push(next_punct.as_char());
                                token
                            } else {
                                punct.as_char().to_string()
                            }
                        } else {
                            punct.as_char().to_string()
                        };

                        if [">=", ">", "<", "<=", "="].contains(&token.as_str())
                        {
                            constraint = Some(token);
                        }
                    } else {
                        lhs.extend(proc_macro::TokenStream::from(
                            tt.to_token_stream(),
                        ));
                    }

                    rest = next;
                }

                Ok(((), rest))
            })
            .unwrap();

        if constraint.is_none() {
            return Err(input.error("Expected a relation"));
        }

        Ok(Equation {
            lhs: syn::parse::<Expr>(lhs)?,
            constraint: match constraint.unwrap().as_str() {
                ">" => Constraint::Gt,
                ">=" => Constraint::Ge,
                "<=" => Constraint::Le,
                "<" => Constraint::Lt,
                "=" => Constraint::Eq,
                _ => unreachable!(),
            },
            rhs: syn::parse::<Expr>(rhs)?,
        })
    }
}

#[proc_macro]
pub fn relations(input: TokenStream) -> TokenStream {
    let punc = Punctuated::<Equation, Token![;]>::parse_terminated
        .parse(input)
        .unwrap();

    let terms =
        punc.iter().map(|Equation { lhs, constraint, rhs }| match constraint {
            Constraint::Eq => quote! {
                engi::system::eq::Equation::new(#lhs, #rhs)
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
