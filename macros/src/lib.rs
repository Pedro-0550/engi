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

// TODO: Support custom Default impls by just disabling Default when any field is non-standard

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
        panic!("#[derive(Model)] only supports structs");
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
                        .parse(quote! {engi::units::Unit::Unitless}.into())
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

    let variable_idents: Vec<Ident> =
        vars.iter().map(|(field, ..)| field.ident.clone().unwrap()).collect();

    let submodel_idents: Vec<Ident> =
        submodels.iter().map(|field| field.ident.clone().unwrap()).collect();

    let interface_idents: Vec<Ident> =
        interfaces.iter().map(|field| field.ident.clone().unwrap()).collect();

    let variable_builder_idents: Vec<Ident> = vars
        .iter()
        .map(|(field, ..)| {
            format_ident!("{}_builder", field.ident.clone().unwrap())
        })
        .collect();

    let submodel_builder_idents: Vec<Ident> = submodels
        .iter()
        .map(|field| format_ident!("{}_builder", field.ident.clone().unwrap()))
        .collect();

    let interface_builder_idents: Vec<Ident> = interfaces
        .iter()
        .map(|field| format_ident!("{}_builder", field.ident.clone().unwrap()))
        .collect();

    let builder_fields = vars
        .iter()
        .map(|(Field { vis, ident, .. }, ..)| {
            quote! {
                #vis #ident: engi::model::VariableBuilder<'s>
            }
        })
        .chain(submodels.iter().map(|Field { vis, ident, ty, .. }| {
            quote! {
                #vis #ident: <<#ty as engi::model::Model>::Registration as engi::model::ModelRegistration>::Builder<'s>
            }
        }))
        .chain(interfaces.iter().map(|Field { vis, ident, .. }| {
            quote! {
                #vis #ident: engi::model::InterfaceBuilder<'s>
            }
        }))
        .collect::<Vec<_>>();

    let constructor = vars
        .iter()
        .map(|(field, desc, unit, shape)| {
            let field_ident = field.ident.clone().unwrap();

            quote! {
                #field_ident: engi::model::Variable::new(
                    engi::symbol::Symbol::new(
                        &format!(
                            "{}.{}",
                            name,
                            stringify!(#field_ident)
                        )
                    )
                    .set_unit(#unit)
                    .set_shape(#shape)
                    .set_desc(#desc.to_owned())
                )
            }
        })
        .chain(interfaces.iter().map(|field| {
            let field_ident = field.ident.clone().unwrap();
            let field_ty = field.ty.clone();

            quote! {
                #field_ident:
                    <#field_ty as engi::model::Interface>::new(
                        &format!(
                            "{}.{}",
                            name,
                            stringify!(#field_ident)
                        )
                    )
            }
        }))
        .chain(submodels.iter().map(|field| {
            let field_ident = field.ident.clone().unwrap();
            let field_ty = field.ty.clone();

            quote! {
                #field_ident:
                    <#field_ty as engi::model::Model>::new(
                        &format!(
                            "{}.{}",
                            name,
                            stringify!(#field_ident)
                        )
                    )
            }
        }))
        .collect::<Vec<_>>();

    let solution_fields = vars
        .iter()
        .map(|(Field { vis, ident, .. }, ..)| {
            quote! {
                #vis #ident: engi::model::Value
            }
        })
        .chain(submodels.iter().map(|Field { vis, ident, ty, .. }| {
            quote! {
                #vis #ident:
                    <#ty as engi::model::Model>::Solution
            }
        }))
        .collect::<Vec<_>>();

    let registration_fields = vars
        .iter()
        .map(|(Field { vis, ident, .. }, ..)| {
            quote! {
                #vis #ident: engi::model::VariableId
            }
        })
        .chain(interfaces.iter().map(|Field { vis, ident, .. }| {
            quote! {
                #vis #ident: engi::model::InterfaceId
            }
        }))
        .chain(submodels.iter().map(|Field { vis, ident, ty, .. }| {
            quote! {
                #vis #ident:
                    <#ty as engi::model::Model>::Registration
            }
        }))
        .collect::<Vec<_>>();

    let solution_ident = format_ident!("{}Solution", ident);
    let builder_ident = format_ident!("{}Builder", ident);
    let registration_ident = format_ident!("{}Registration", ident);

    quote! {
        #vis struct #builder_ident<'s> {
            system: &'s engi::model::System,
            id: engi::model::ModelId,
            #(#builder_fields,)*
        }

        impl #impl_generics engi::model::Model for #ident #ty_generics #where_clause {
            type Builder<'s> = #builder_ident<'s>;
            type Solution = #solution_ident;

            fn builder<'s>(id: engi::model::ModelId, system: &'s System) -> Self::Builder<'s> {
                #builder_ident {

                }
            }
        }

        #vis struct #solution_ident {
            #(#solution_fields,)*
        }

        impl #impl_generics engi::model::Solution for #ident #ty_generics #where_clause {
            fn disassemble(assembled: engi::model::AssembledSolution) -> Self {
                Self {
                    #(#variable_idents:)
                }
            }

        }

        // impl #impl_generics engi::model::Model for #ident #ty_generics #where_clause {
        //     type Solution = #solution_ident;
        //     type Registration = #registration_ident;

        //     fn new(name: &str) -> #ident {
        //         #ident {
        //             #(#constructor,)*
        //         }
        //     }

        //     fn register(self, system: &mut System) -> Self::Registration {
        //         #(let #variable_builder_idents = system.add_variable(self.#variable_idents.clone());)*
        //         #(let #interface_builder_idents = system.add_interface(self.#interface_idents.clone());)*

        //         #(let #submodel_builder_idents = self.#submodel_idents.clone().register(system);)*

        //         let own_id = system.add_model(self);

        //         #registration_ident {
        //             id: own_id,
        //             #(#submodel_idents: #submodel_builder_idents,)*
        //             #(#interface_idents: #interface_builder_idents,)*
        //             #(#variable_idents: #variable_builder_idents,)*
        //         }
        //     }
        // }
    }.into()
}

/* -------------------------------------------------------------------------- */

struct ConnectAttr {
    condition: Expr,
    unit: Expr,
    desc: LitStr,
    shape: Expr,
}

impl Default for ConnectAttr {
    fn default() -> Self {
        Self {
            desc: LitStr::new("", Span::call_site()),
            unit: Expr::parse
                .parse(quote! {engi::units::Unit::Unitless}.into())
                .unwrap(),

            shape: Expr::parse
                .parse(quote! {engi::expr::Shape::SCALAR}.into())
                .unwrap(),
            condition: Expr::parse
                .parse(quote! {engi::model::Condition::Equal}.into())
                .unwrap(),
        }
    }
}

impl ConnectAttr {
    fn parse(attr: &syn::Attribute) -> syn::Result<Self> {
        let mut s = Self::default();

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("unit") {
                s.unit = meta.value()?.parse()?;
            } else if meta.path.is_ident("desc") {
                s.desc = meta.value()?.parse()?;
            } else if meta.path.is_ident("shape") {
                s.shape = meta.value()?.parse()?
            } else if meta.path.is_ident("cond") {
                s.condition = meta.value()?.parse()?;
            } else {
                return Err(meta.error("unknown #[connect] argument"));
            }

            Ok(())
        })?;

        Ok(s)
    }
}

#[proc_macro_derive(Interface, attributes(connect))]
pub fn interface(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let DeriveInput { vis, ident, generics, data, .. } =
        parse_macro_input!(input as DeriveInput);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let Data::Struct(DataStruct { fields, .. }) = data else {
        panic!("#[derive(Interface)] only supports structs");
    };

    let fields = fields.iter().map(|f| {
        let attr = f
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("connect"))
            .at_most_one()
            .map_err(|_| "()")
            .expect("A field cannot have more than one #[connect] attr");

        let attr = attr
            .map(ConnectAttr::parse)
            .map(|r| r.unwrap())
            .unwrap_or_default();

        (f, attr)
    });

    let connector_exprs = fields.clone().map(|(field, _)| field.ident.clone());

    let constructor = fields.map(|(field, attr)| {
        let ConnectAttr { condition, unit, desc, shape } = attr;
        let field_ident = field.ident.clone().unwrap();
        quote! {
            #field_ident: engi::model::Connector::new(engi::model::Variable::new(
                engi::symbol::Symbol::new(&format!("{}.{}", name, stringify!(#field_ident)))
                    .set_unit(#unit)
                    .set_shape(#shape)
                    .set_desc(#desc.to_owned())
            ), #condition)
        }
    });

    quote! {
        impl #impl_generics engi::model::Interface for #ident #ty_generics #where_clause {
            fn new(name: &str) -> Self {
                #ident {
                    #(#constructor,)*
                }
            }

            fn connectors(&self) -> Vec<engi::model::Connector> {
                vec![
                    #(self.#connector_exprs,)*
                ]
            }
        }
    }
    .into()
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
                engi::model::eq::Equation::new(#lhs, #rhs)
            },
            Constraint::Gt => quote! {
                engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::Greater)
            },
            Constraint::Ge => quote! {
                engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::GreaterOrEq)
            },
            Constraint::Lt => quote! {
                engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::Less)
            },
            Constraint::Le => quote! {
                engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::LessOrEq)
            },
        });

    quote! {vec![#(#terms),*]}.into()
}

#[proc_macro]
pub fn relation(input: TokenStream) -> TokenStream {
    let Equation { lhs, constraint, rhs } =
        Equation::parse.parse(input).unwrap();

    let expr = match constraint {
        Constraint::Eq => quote! {
            engi::model::eq::Equation::new(#lhs, #rhs)
        },
        Constraint::Gt => quote! {
            engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::Greater)
        },
        Constraint::Ge => quote! {
            engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::GreaterOrEq)
        },
        Constraint::Lt => quote! {
            engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::Less)
        },
        Constraint::Le => quote! {
            engi::model::eq::Constraint::new(#lhs, #rhs, engi::model::eq::Inequality::LessOrEq)
        },
    };

    quote! { #expr }.into()
}
