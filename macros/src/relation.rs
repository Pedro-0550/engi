use proc_macro::TokenStream;
use proc_macro2::Spacing;
use syn::{
    Expr, Token,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
};
use template_quote::{ToTokens, quote};

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

pub fn relations(input: TokenStream) -> TokenStream {
    let punc = Punctuated::<Equation, Token![;]>::parse_terminated
        .parse(input)
        .unwrap();
    // TODO: update constraints to have a similar conversion as equations
    let terms =
        punc.iter().map(|Equation { lhs, constraint, rhs }| match constraint {
            Constraint::Eq => quote! {
                engi::model::eq::Equations::from((
                    #lhs,
                    #rhs
                )).0
            },
            Constraint::Gt => quote! {
                engi::model::eq::Constraints::from((
                    #lhs,
                    #rhs,
                    engi::model::eq::Inequality::Greater
                )).0
            },
            Constraint::Ge => quote! {
                engi::model::eq::Constraints::from((
                    #lhs,
                    #rhs,
                    engi::model::eq::Inequality::GreaterOrEq
                )).0
            },
            Constraint::Lt => quote! {
                engi::model::eq::Constraints::from((
                    #lhs,
                    #rhs,
                    engi::model::eq::Inequality::Less
                )).0
            },
            Constraint::Le => quote! {
                engi::model::eq::Constraints::from((
                    #lhs,
                    #rhs,
                    engi::model::eq::Inequality::LessOrEq
                )).0
            },
        });

    quote! {[#(#terms),*].concat()}.into()
}

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
