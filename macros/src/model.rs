use itertools::Itertools;
// use proc_macro::ToTokens;
use proc_macro2::{Span, TokenStream};
use syn::{
    Attribute, Data, DataStruct, DeriveInput, Expr, Field, GenericArgument,
    Generics, Ident, Item, ItemStruct, LitStr, PathArguments, Type, Visibility,
    parse::{Parse, Parser},
    parse_macro_input,
    token::Token,
};
use template_quote::{ToTokens, quote};

use crate::{get_stateful, states::emit_view, suffix_ident};

/* -------------------------------------------------------------------------- */

#[derive(Clone)]
pub struct VariableAttr {
    unit: Expr,
    desc: LitStr,
    shape: Expr,
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

        Ok(Self {
            unit: unit.unwrap_or(
                Expr::parse
                    .parse(quote! { engi::unit::defs::UNITLESS }.into())?,
            ),
            desc: desc.unwrap_or(LitStr::new("", Span::call_site())),
            shape: shape.unwrap_or(
                Expr::parse.parse(
                    quote! { engi::expr::shape::Shape::SCALAR }.into(),
                )?,
            ),
        })
    }
}

/* -------------------------------------------------------------------------- */

#[derive(Clone)]
enum ModelField {
    Submodel(Field),
    Interface(Field),
    Variable(Field, VariableAttr),
    Stateful { s: Type, inner: Box<ModelField> },
}

impl ModelField {
    fn vis(&self) -> Visibility {
        match self {
            ModelField::Submodel(f)
            | ModelField::Interface(f)
            | ModelField::Variable(f, ..) => f.vis.clone(),
            ModelField::Stateful { s, inner } => inner.vis(),
        }
    }

    fn ident(&self) -> Ident {
        match self {
            ModelField::Submodel(f)
            | ModelField::Interface(f)
            | ModelField::Variable(f, ..) => f.ident.clone().unwrap(),
            ModelField::Stateful { s, inner } => inner.ident(),
        }
    }

    fn ty(&self) -> Type {
        match self {
            ModelField::Submodel(f)
            | ModelField::Interface(f)
            | ModelField::Variable(f, ..) => f.ty.clone(),
            ModelField::Stateful { s, inner } => Type::parse.parse2(
                quote! { engi::model::state::Stateful<#{ inner.ty() }, #s> },
            ).unwrap(),
        }
    }

    fn retyped(&self, mut ty_f: impl FnMut(Self) -> TokenStream) -> Field {
        Field::parse_named
            .parse2(
                quote! {#{&self.vis()} #{&self.ident()}: #{ty_f(self.clone())}},
            )
            .unwrap()
    }

    fn field(&self) -> Field {
        Field::parse_named
            .parse2(quote! {#{&self.vis()} #{&self.ident()}: #{&self.ty()}})
            .unwrap()
    }
}

impl TryFrom<Field> for ModelField {
    type Error = syn::Error;

    fn try_from(f: Field) -> Result<Self, Self::Error> {
        let mut model_field = None;

        if let Some((s, inner)) = get_stateful(&f)? {
            return Ok(ModelField::Stateful {
                s,
                inner: Box::new(Self::try_from(inner)?),
            });
        }

        for attr in &f.attrs {
            let Some(ident) = attr.path().get_ident() else {
                continue;
            };

            let curr_field = match ident.to_string().as_str() {
                "interface" => Some(ModelField::Interface(f.clone())),
                "model" => Some(ModelField::Submodel(f.clone())),
                "var" => Some(ModelField::Variable(
                    f.clone(),
                    VariableAttr::parse(&attr)?,
                )),
                _ => None,
            };

            match (&mut model_field, curr_field) {
                (None, Some(f)) => {
                    model_field.insert(f);
                }
                (Some(_), Some(_)) => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "Found multiple modeling attrs on the same field",
                    ));
                }
                _ => (),
            };
        }

        model_field.ok_or(syn::Error::new_spanned(
            f,
            "You need to specify this model field kind with #[interface], #[model], or #[var(..)] attrs",
        ))
    }
}

/* -------------------------------------------------------------------------- */

pub struct Model {
    vis: Visibility,
    ident: Ident,
    generics: Generics,
    name: Field,
    fields: Vec<ModelField>,
}

impl Parse for Model {
    fn parse(buffer: syn::parse::ParseStream) -> syn::Result<Self> {
        let model_struct = ItemStruct::parse(buffer)?;
        let ItemStruct { vis, ident, generics, fields, .. } =
            model_struct.clone();

        let mut name = None;
        let mut model_fields = Vec::new();

        for f in fields {
            if let Some(ref ident) = f.ident
                && ident == "name"
            {
                if let Type::Path(ref p) = f.ty
                    && let p = p.to_token_stream().to_string()
                    && p != "String"
                    && p != "std::string::String"
                {
                    return Err(syn::Error::new_spanned(
                        f,
                        "Invalid name field type, should be String",
                    ));
                };

                name = Some(f);
            } else {
                model_fields.push(ModelField::try_from(f)?)
            }
        }

        let name = name.ok_or(syn::Error::new_spanned(
            model_struct,
            "Models need a `name: String` field",
        ))?;

        Ok(Model { vis, ident, generics, name, fields: model_fields })
    }
}

/* -------------------------------------------------------------------------- */

fn emit_solution(m: &Model) -> Result<TokenStream, syn::Error> {
    fn solution_ty(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::Solution }
            }

            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::Solution }
            }

            ModelField::Variable(f, _) => quote! { engi::unit::Quantity },

            ModelField::Stateful { s, inner } => {
                let ty = solution_ty(*inner.clone());
                quote! { engi::model::state::Stateful<#ty, #s> }
            }
        }
    }

    fn solution_disassembly(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! {
                    let sol = <<#{f.ty} as engi::model::Model>::Solution as engi::model::solve::ModelSolution>::disassemble(assembled.submodels[cursor.submodel].clone());
                    cursor.submodel += 1;
                    sol
                }
            }
            ModelField::Interface(f) => quote! {
                let sol = <<#{f.ty} as engi::model::Interface>::Solution as engi::model::solve::InterfaceSolution>::disassemble(assembled.interfaces[cursor.interface].clone());
                cursor.interface += 1;
                sol
            },
            ModelField::Variable(..) => quote! {
                let sol = assembled.variables[cursor.variable].clone();
                cursor.variable += 1;
                sol
            },
            ModelField::Stateful { s, inner } => quote! {
                engi::model::state::Stateful::from_fn(|_| { #{ solution_disassembly(*inner.clone()) } })
            },
        }
    }

    let solution_ident = suffix_ident(&m.ident, "Solution");
    let view_fields =
        m.fields.iter().map(|f| f.retyped(solution_ty)).collect_vec();

    Ok(quote! {
        #[derive(Clone, Default)]
        #{ &m.vis } struct #solution_ident {
            #{&m.name.ident}: String,
            #(for f in &m.fields) {
                #{&f.vis()} #{&f.ident()}: #{&solution_ty(f.clone())},
            }
        }

        #{&emit_view(&m.vis, &solution_ident, &Generics::default(), &view_fields)?}


        impl engi::model::solve::ModelSolution for #solution_ident {
            fn disassemble(mut assembled: engi::model::solve::AssembledModelSolution) -> Self {
                let mut cursor = engi::model::ModelCursor::default();

                Self {
                    #{&m.name.ident}: assembled.name,
                    #(for f in &m.fields) {
                        #{&f.ident()}: { #{&solution_disassembly(f.clone())} },
                    }
                }
            }
        }
    })
}

fn emit_builder(m: &Model) -> Result<TokenStream, syn::Error> {
    fn builder_ty(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::Builder<'s> }
            }

            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::Builder<'s> }
            }
            ModelField::Variable(f, _) => {
                quote! { engi::model::VariableBuilder<'s> }
            }
            ModelField::Stateful { s, inner } => {
                let ty = builder_ty(*inner.clone());
                quote! { engi::model::state::Stateful<#ty, #s> }
            }
        }
    }

    fn template_bind(f: ModelField, access: TokenStream) -> TokenStream {
        match f {
            ModelField::Variable(f, _) => quote! {
                if let Some(_v) = &template.#{&f.ident} #access {
                    self.#{&f.ident} #access .bind(_v);
                }
            },
            ModelField::Submodel(f) => quote! {
                engi::model::ModelBuilder::bind(&self.#{&f.ident} #access, &template.#{&f.ident} #access);
            },
            ModelField::Interface(f) => quote! {
                engi::model::InterfaceBuilder::bind(&self.#{&f.ident} #access, &template.#{&f.ident} #access);
            },

            ModelField::Stateful { s, inner } => quote! {
                for __var in <#s as engi::model::state::States>::VARIANTS {
                    #{&template_bind(*inner.clone(), quote!{[*__var]})}
                }
            },
        }
    }

    let builder_ident = suffix_ident(&m.ident, "Builder");

    let view_fields =
        m.fields.iter().map(|f| f.retyped(builder_ty)).collect_vec();
    let builder_generics = Generics::parse.parse2(quote! {<'s>})?;

    Ok(quote! {
        #[derive(Clone)]
        #{&m.vis} struct #builder_ident<'s> {
            path: engi::model::ModelPath,
            #(for f in &m.fields) {
                #{&f.vis()} #{&f.ident()}: #{&builder_ty(f.clone())},
            }
        }

        #{emit_view(&m.vis, &builder_ident, &builder_generics, &view_fields)?}

        impl<'s> engi::model::ModelBuilder for #builder_ident<'s> {
            type Builds = #{&m.ident};

            fn path(&self) -> &engi::model::ModelPath {
                &self.path
            }

            fn bind(&self, template: &<#{&m.ident} as engi::model::Model>::Template) {
                use engi::model::ModelBuilder;
                use std::ops::Index;

                #(for f in &m.fields) {
                    { #{&template_bind(f.clone(), quote!{})} };
                }
            }
        }
    })
}

fn emit_template(m: &Model) -> Result<TokenStream, syn::Error> {
    let template_ident = suffix_ident(&m.ident, "Template");

    fn template_ty(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::Template }
            }
            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::Template }
            }
            ModelField::Variable(f, _) => {
                quote! { Option<engi::unit::Quantity> }
            }
            ModelField::Stateful { s, inner } => {
                let ty = template_ty(*inner.clone());
                quote! { engi::model::state::Stateful<#ty, #s> }
            }
        }
    }

    let view_fields =
        m.fields.iter().map(|f| f.retyped(template_ty)).collect_vec();

    Ok(quote! {
        #[derive(Default, Clone)]
        #{&m.vis} struct #template_ident {
            #(for f in &m.fields) {
                #{&f.vis()} #{&f.ident()}: #{&template_ty(f.clone())},
            }
        }

        #{emit_view(&m.vis, &template_ident, &Generics::default(), &view_fields)?}

        impl engi::model::ModelTemplate for #template_ident {}
    })
}

fn emit_model_impl(m: &Model) -> TokenStream {
    let template_ident = suffix_ident(&m.ident, "Template");
    let builder_ident = suffix_ident(&m.ident, "Builder");
    let solution_ident = suffix_ident(&m.ident, "Solution");
    let (impl_generics, ty_generics, where_clause) =
        m.generics.split_for_impl();

    fn constructor(f: ModelField, suffix: TokenStream) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::new(&format!("{}.{}{}", name, stringify!(#{&f.ident}), #suffix)) }
            }
            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::new(&format!("{}.{}{}", name, stringify!(#{&f.ident}), #suffix)) }
            }
            ModelField::Variable(f, variable_attr) => quote! {
                engi::model::Variable::new(
                    engi::symbol::Symbol::builder()
                        .name(format!("{}.{}{}", name, stringify!(#{&f.ident}), #suffix).leak())
                        .shape(#{&variable_attr.shape})
                        .unit(#{&variable_attr.unit})
                        .desc(#{&variable_attr.desc})
                        .build()
                )
            },
            ModelField::Stateful { s, inner } => {
                let expr = constructor(
                    *inner.clone(),
                    quote! {format!("[{}]{}", engi::model::state::States::name(&__state), #suffix)},
                );

                quote! {
                    engi::model::state::Stateful::<_, #s>::from_fn(|__state| {
                        #expr
                    })
                }
            }
        }
    }

    fn builder(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => quote! {
                let builder = <#{&f.ty} as engi::model::Model>::builder(path.with(&[cursor.submodel]), system);
                cursor.submodel += 1;
                builder
            },
            ModelField::Interface(f) => quote! {
                let builder = <#{&f.ty} as engi::model::Interface>::builder(
                    engi::model::InterfaceId::new(Clone::clone(&path), cursor.interface),
                    system
                );
                cursor.interface += 1;
                builder
            },
            ModelField::Variable(..) => quote! {
                let builder = engi::model::VariableBuilder::new(
                    engi::model::VariableId::new(Clone::clone(&path), cursor.variable),
                    system
                );
                cursor.variable += 1;
                builder
            },
            ModelField::Stateful { s, inner } => quote! {
                engi::model::state::Stateful::from_fn(|_| { #{ builder(*inner.clone()) } })
            },
        }
    }

    fn assemble(f: ModelField, access: TokenStream) -> TokenStream {
        match f {
            ModelField::Submodel(f) => quote! {
                submodels.push(<#{&f.ty} as engi::model::Model>::assemble(#access))
            },
            ModelField::Interface(f) => quote! {
                interfaces.push(<#{&f.ty} as engi::model::Interface>::assemble(#access))
            },
            ModelField::Variable(..) => quote! {
                variables.push(#access)
            },
            ModelField::Stateful { s, inner } => quote! {
                for __state in self.#{&inner.ident()} {
                    #{&assemble(*inner.clone(), quote!{__state})}
                }
            },
        }
    }

    quote! {
        impl #impl_generics engi::model::Model #ty_generics for #{&m.ident} #where_clause {
            type Template = #template_ident;
            type Solution = #solution_ident;
            type Builder<'s> = #builder_ident<'s>;

            fn new(name: &str) -> Self {
                Self {
                    #(for f in &m.fields) {
                        #{&f.ident()}: { #{&constructor(f.clone(), quote!{""})} },
                    }
                    #{&m.name.ident}: name.to_owned(),
                }
            }

            fn builder<'s>(path: engi::model::ModelPath, system: &'s engi::model::System) -> Self::Builder<'s> {
                let mut cursor = engi::model::ModelCursor::default();

                Self::Builder {
                    #(for f in &m.fields) {
                        #{&f.ident()}: { #{&builder(f.clone())} },
                    }
                    path,
                }
            }

            fn assemble(self) -> engi::model::AssembledModel {
                let (mut variables, mut interfaces, mut submodels) = (Vec::new(), Vec::new(), Vec::new());
                let equations = <Self as engi::model::Relations>::equations(&self);
                let constraints = <Self as engi::model::Relations>::constraints(&self);

                #(for f in &m.fields) {
                    { #{&assemble(f.clone(), quote!{ self.#{&f.ident()} })} };
                }

                engi::model::AssembledModel {
                    variables, interfaces, submodels, equations, constraints,
                    name: self.#{&m.name.ident},
                }
            }
        }
    }
}

pub fn derive_model(
    input: proc_macro::TokenStream,
) -> Result<proc_macro::TokenStream, syn::Error> {
    let m = Model::parse.parse(input)?;
    let view_fields = m.fields.iter().map(ModelField::field).collect_vec();

    Ok(quote! {
        #{&emit_view(&m.vis, &m.ident, &m.generics, &view_fields)?}
        #{&emit_builder(&m)?}
        #{&emit_solution(&m)?}
        #{&emit_template(&m)?}

        #{&emit_model_impl(&m)}
    }
    .into())
}
