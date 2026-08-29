use std::iter::once;

use darling::{FromDeriveInput, FromField, FromMeta, ast, util};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DataStruct, DeriveInput, Field, Ident, Type, parse_macro_input,
};

#[derive(Debug, FromField)]
#[darling(attributes(var))]
struct VariableField {
    ident: Option<Ident>,
    ty: Type,
    #[darling(default)]
    skip: bool,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(var), supports(struct_named))]
struct System {
    ident: Ident,
    data: ast::Data<util::Ignored, VariableField>,
}

// #[derive(FromMeta)]
// struct SystemAttr {
//     unit:
// }

#[proc_macro_derive(System)]
pub fn system_derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let DeriveInput { attrs, vis, ident, generics, data } =
        parse_macro_input!(input as DeriveInput);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let Data::Struct(DataStruct { fields, .. }) = data else {
        panic!("#[derive(System)] only supports enums");
    };

    let (mut variables, mut subsystems, mut inputs) =
        (Vec::new(), Vec::new(), Vec::new());

    for field in fields {
        let Type::Path(ref p) = field.ty else {
            panic!(
                "System structs can only hold Variable, Solution, or another System."
            );
        };

        let ident = p.path.segments.last().unwrap().ident.clone();

        if ident == "Variable" {
            variables.push(field);
        } else if ident == "Solution" {
            inputs.push(field);
        } else {
            subsystems.push(field);
        }
    }

    let variable_idents = variables.iter().cloned().map(|f| f.ident);
    let variables_impl = quote! {
        impl #impl_generics engi::system::Variables for #ident #ty_generics #where_clause {
            fn variables(&self) -> Vec<engi::system::var::Variable> {
                vec![
                    #(self.#variable_idents),*
                ]
            }
        }
    };

    let solution_ident = format_ident!("{}Solution", ident);
    let solution_fields =
        variables.iter().map(|Field { attrs, vis, ident, .. }| {
            quote! {
                #(#attrs)*
                #vis #ident: engi::core::scalar::Scalar
            }
        });

    let subsystem_tys = subsystems.iter().cloned().map(|f| f.ty);

    let [collect_vars_terms, collect_eq_terms, collect_cons_terms] = [
        (quote! {collect_variables}, quote! {variables}),
        (quote! {collect_equations}, quote! {equations}),
        (quote! {collect_constraints}, quote! {constraints}),
    ]
    .map(|(func, base_fn)| {
        subsystems
            .iter()
            .map(move |Field { ident, .. }| {
                quote! {
                    self.#ident.#func(into)
                }
            })
            .chain(once(quote! {
                into.append(&mut self.#base_fn())
            }))
    });

    let system_impl = quote! {
        type Subsystems = ( #(#subsystem_tys),* );

        #vis struct #solution_ident {
            #(#solution_fields),*
        }

        impl #impl_generics System for #ident #ty_generics #where_clause {
            type Solution = #solution_ident;

            fn collect_variables(&self, into: &mut Vec<engi::system::var::Variable>) {
                #(#collect_vars_terms);*
            }
            fn collect_equations(&self, into: &mut Vec<engi::system::eq::Equation>) {
                #(#collect_eq_terms);*
            }
            fn collect_constraints(&self, into: &mut Vec<engi::system::eq::Constraint>) {
                #(#collect_cons_terms);*
            }
        }
    };

    quote! {
        #variables_impl
        #system_impl
    }
    .into()
}
