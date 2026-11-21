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
            unit: unit.unwrap_or(Expr::parse.parse(
                "engi::unit::defs::UNITLESS".to_token_stream().into(),
            )?),
            desc: desc.unwrap_or(LitStr::new("", Span::call_site())),
            shape: shape.unwrap_or(Expr::parse.parse(
                "engi::expr::shape::Shape::SCALAR".to_token_stream().into(),
            )?),
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
}

impl TryFrom<Field> for ModelField {
    type Error = syn::Error;

    fn try_from(f: Field) -> Result<Self, Self::Error> {
        let mut model_field = None;

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

            return Ok(ModelField::Stateful {
                s: s.clone(),
                inner: Box::new(Self::try_from(inner_field)?),
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
    attrs: Vec<Attribute>,
    fields: Vec<ModelField>,
}

impl Parse for Model {
    fn parse(buffer: syn::parse::ParseStream) -> syn::Result<Self> {
        let model_struct = ItemStruct::parse(buffer)?;
        let ItemStruct { attrs, vis, ident, generics, fields, .. } =
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

        Ok(Model { vis, ident, generics, name, attrs, fields: model_fields })
    }
}

/* -------------------------------------------------------------------------- */

fn emit_solution(m: &Model) -> TokenStream {
    let solution_ident =
        Ident::new(&format!("{}Solution", &m.ident), Span::call_site());

    fn solution_ty(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::Solution, }
            }

            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::Solution, }
            }

            ModelField::Variable(f, _) => quote! { engi::unit::Quantity, },

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
                    let sol = <#{f.ty} as Model>::Solution::disassemble(assembled.submodels[cursor.submodel])
                    cursor.submodel += 1;
                    sol
                }
            }
            ModelField::Interface(f) => quote! {
                let sol = <#{f.ty} as Interface>::Solution::disassemble(assembled.interfaces[cursor.submodel])
                cursor.submodel += 1;
                sol
            },
            ModelField::Variable(..) => quote! {
                let sol = assembled.variables[cursor.variable];
                cursor.variable += 1;
                sol
            },
            ModelField::Stateful { s, inner } => quote! {
                engi::model::state::Stateful::from_fn(|_| { #{ solution_disassembly(*inner.clone()) } })
            },
        }
    }

    quote! {
        #[derive(Clone)]
        #{ &m.vis } struct #solution_ident {
            name: String,
            #(for f in &m.fields) {
                #{&f.vis()} #{&f.ident()}: #{&solution_ty(f.clone())}
            }
        }

        impl engi::model::ModelSolution for #{ &m.ident } {
            fn disassemble(assembled: AssembledModelSolution) -> Self {
                let mut cursor = engi::model::ModelCursor::default();

                Self {
                    name: assembled.name,
                    #(for f in &m.fields) {
                        #{&f.ident()}: { #{&solution_disassembly(f.clone())} }
                    }
                }
            }
        }
    }
}

fn emit_builder(m: &Model) -> TokenStream {
    let builder_ident =
        Ident::new(&format!("{}Builder", &m.ident), Span::call_site());

    fn builder_ty(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::Builder<'s>, }
            }

            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::Builder<'s>, }
            }
            ModelField::Variable(f, _) => {
                quote! { engi::model::VariableBuilder<'s>, }
            }
            ModelField::Stateful { s, inner } => {
                let ty = builder_ty(*inner.clone());
                quote! { engi::model::state::Stateful<#ty, #s> }
            }
        }
    }

    fn template_bind(f: ModelField) -> TokenStream {
        match f {
            ModelField::Variable(f, _) => quote! {
                if let Some(ref _v) = template.#{&f.ident} {
                    self.#{&f.ident}.bind(_v);
                }
            },
            ModelField::Submodel(f) | ModelField::Interface(f) => quote! {
                self.#{&f.ident}.bind(template.#{&f.ident});
            },

            ModelField::Stateful { s, inner } => template_bind(*inner.clone()),
        }
    }

    quote! {
        #{&m.vis} struct #builder_ident<'s> {
            path: engi::model::ModelPath,
            #(for f in &m.fields) {
                #{&f.vis()} #{&f.ident()}: #{&builder_ty(f.clone())}
            }
        }

        impl<'s> engi::model::ModelBuilder for #builder_ident<'s> {
            type Builds = #{&m.ident};

            fn path(&self) -> &engi::model::ModelPath {
                &self.path
            }

            fn bind(&self, template: <#{&m.ident} as engi::model::Model>::Template) {
                use engi::model::ModelBuilder;

                #(for f in &m.fields) {
                    #{&template_bind(f.clone())}
                }
            }
        }
    }
}

fn emit_template(m: &Model) -> TokenStream {
    let template_ident =
        Ident::new(&format!("{}Template", &m.ident), Span::call_site());

    fn template_ty(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::Template, }
            }
            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::Template, }
            }
            ModelField::Variable(f, _) => {
                quote! { Option<engi::unit::Quantity>, }
            }
            ModelField::Stateful { s, inner } => {
                let ty = template_ty(*inner.clone());
                quote! { engi::model::state::Stateful<#ty, #s> }
            }
        }
    }

    quote! {
        #[derive(Default, Clone)]
        #{&m.vis} struct #template_ident {
            #(for f in &m.fields) {
                #{&f.vis()} #{&f.ident()}: #{&template_ty(f.clone())}
            }
        }

        impl engi::model::ModelTemplate for #template_ident {}
    }
}

fn emit_view(m: &Model) -> TokenStream {
    let view_ident =
        Ident::new(&format!("{}StatefulView", &m.ident), Span::call_site());

    fn view_ty(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { engi::model::state::Stateful<#{&f.ty}, S>, }
            }
            ModelField::Interface(f) => {
                quote! { engi::model::state::Stateful<#{&f.ty}, S>, }
            }
            ModelField::Variable(f, _) => {
                quote! { engi::model::state::Stateful<#{&f.ty}, S>, }
            }
            ModelField::Stateful { s, inner } => {
                let inner_ty = inner.ty();
                quote! { engi::model::state::Stateful<engi::model::state::Stateful<#inner_ty, #s>, S> }
            }
        }
    }

    quote! {
        #[derive(Default, Clone)]
        #{&m.vis} struct #view_ident<S> where S: engi::model::state::States {
            #(for f in &m.fields) {
                #{&f.vis()} #{&f.ident()}: #{&view_ty(f.clone())}
            }
        }

        impl<S> engi::model::state::StatefulView<S> for #view_ident
        where S: engi::model::state::States {
            type Of = #{&m.ident};

            fn new(states: &[Self::Of]) -> Self {
                Self {
                    #(for f in &m.fields) {
                        #{&f.ident()}: engi::model::Stateful::new(::array::from_fn(|i| states[i].#{&f.ident()}.clone()))
                    }
                }
            }
        }
    }
}

fn emit_model_impl(m: &Model) -> TokenStream {
    let view_ident =
        Ident::new(&format!("{}StatefulView", &m.ident), Span::call_site());
    let template_ident =
        Ident::new(&format!("{}Template", &m.ident), Span::call_site());
    let builder_ident =
        Ident::new(&format!("{}Builder", &m.ident), Span::call_site());
    let solution_ident =
        Ident::new(&format!("{}Solution", &m.ident), Span::call_site());

    fn constructor(f: ModelField, suffix: TokenStream) -> TokenStream {
        match f {
            ModelField::Submodel(f) => {
                quote! { <#{&f.ty} as engi::model::Model>::new(format!("{}.{}{}", name, stringify!(#{&f.ident}), #suffix)) }
            }
            ModelField::Interface(f) => {
                quote! { <#{&f.ty} as engi::model::Interface>::new(format!("{}.{}{}", name, stringify!(#{&f.ident}), #suffix)) }
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
                let expr = constructor(*inner.clone(), quote! {s});

                quote! {
                    engi::model::state::Stateful::from_fn(|s| {
                        #expr
                    })
                }
            }
        }
    }

    fn builder(f: ModelField) -> TokenStream {
        match f {
            ModelField::Submodel(f) => quote! {
                let builder = <#{&f.ty} as engi::model::Model>::builder(path.with(cursor.submodel), system);
                cursor.submodel += 1;
                builder
            },
            ModelField::Interface(f) => quote! {
                let builder = <#{&f.ty} as engi::model::Interface>::builder(
                    engi::model::InterfaceId::new(path.clone(), cursor.interface),
                    system
                )
                cursor.interface += 1;
                builder
            },
            ModelField::Variable(..) => quote! {
                let builder = VariableBuilder::new(
                    engi::model::VariableId::new(path.clone(), cursor.variable),
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
                for s in #{&inner.ident()} {
                    #{&assemble(*inner.clone(), quote!{s})}
                }
            },
        }
    }

    quote! {
        impl engi::model::Model for #{&m.ident} {
            type Template = #template_ident;
            type Solution = #solution_ident;
            type Builder<'s> = #builder_ident<'s>;
            type StatefulView<S: States> = #view_ident<S>;

            fn new(name: &str) -> Self {
                Self {
                    name: name.to_owned(),
                    #(for f in &m.fields) {
                        #{&f.ident()}: { #{&constructor(f.clone(), quote!{""})} }
                    }
                }
            }

            fn builder<'s>(path: ModelPath, system: &'s System) -> Self::Builder<'s> {
                let mut cursor = engi::model::ModelCursor::default();

                Self::Builder {
                    path,
                    #(for f in &m.fields) {
                        #{&f.ident()}: { #{&builder(f.clone())} }
                    }
                }
            }

            fn assemble(self) -> AssembledModel {
                let (mut variables, mut interfaces, mut submodels) = (Vec::new(), Vec::new(), Vec::new());

                #(for f in &m.fields) {
                    #{&assemble(f.clone(), quote!{ self.#{&f.ident()} })}
                }

                engi::model::AssembledModel {
                    name: self.name,
                    equations: <Self as engi::model::Relations>::equations(&self),
                    constraints: <Self as engi::model::Relations>::constraints(&self),
                    variables, interfaces, submodels
                }
            }
        }
    }
}

fn derive_model(input: TokenStream) -> TokenStream {}
