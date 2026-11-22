use proc_macro2::{Span, TokenStream};
use syn::{
    Attribute, Expr, Field, Generics, Ident, ItemStruct, LitStr, Type,
    Visibility,
    parse::{Parse, Parser},
};
use template_quote::quote;

use crate::{get_stateful, suffix_ident};

/* -------------------------------------------------------------------------- */

#[derive(Clone)]
struct ConnectorAttr {
    condition: Expr,
    unit: Expr,
    desc: LitStr,
    shape: Expr,
}

impl Default for ConnectorAttr {
    fn default() -> Self {
        Self {
            desc: LitStr::new("", Span::call_site()),
            unit: Expr::parse
                .parse(quote! {engi::unit::defs::UNITLESS}.into())
                .unwrap(),

            shape: Expr::parse
                .parse(quote! {engi::expr::shape::Shape::SCALAR}.into())
                .unwrap(),
            condition: Expr::parse
                .parse(quote! {engi::model::Condition::Equal}.into())
                .unwrap(),
        }
    }
}

impl ConnectorAttr {
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

/* -------------------------------------------------------------------------- */

#[derive(Clone)]
enum InterfaceField {
    // I plan on adding subinterfaces soon
    // Subinterface(Field),
    Connector(Field, ConnectorAttr),
    Stateful { s: Type, inner: Box<InterfaceField> },
}

impl InterfaceField {
    fn vis(&self) -> Visibility {
        match self {
            InterfaceField::Connector(f, ..) => f.vis.clone(),
            InterfaceField::Stateful { s, inner } => inner.vis(),
        }
    }

    fn ident(&self) -> Ident {
        match self {
            InterfaceField::Connector(f, ..) => f.ident.clone().unwrap(),
            InterfaceField::Stateful { s, inner } => inner.ident(),
        }
    }

    fn ty(&self) -> Type {
        match self {
            InterfaceField::Connector(f, ..) => f.ty.clone(),
            InterfaceField::Stateful { s, inner } => Type::parse.parse2(
                quote! { engi::model::state::Stateful<#{ inner.ty() }, #s> },
            ).unwrap(),
        }
    }
}

impl TryFrom<Field> for InterfaceField {
    type Error = syn::Error;

    fn try_from(f: Field) -> Result<Self, Self::Error> {
        let mut interface_field = None;

        if let Some((s, inner)) = get_stateful(&f)? {
            return Ok(InterfaceField::Stateful {
                s,
                inner: Box::new(Self::try_from(inner)?),
            });
        }

        for attr in &f.attrs {
            let Some(ident) = attr.path().get_ident() else {
                continue;
            };

            let curr_field = match ident.to_string().as_str() {
                "connect" | "conn" => Some(InterfaceField::Connector(
                    f.clone(),
                    ConnectorAttr::parse(&attr)?,
                )),
                // "interface" => Some(InterfaceField::Subinterface(f.clone())),
                _ => None,
            };

            match (&mut interface_field, curr_field) {
                (None, Some(f)) => {
                    interface_field.insert(f);
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

        interface_field.ok_or(syn::Error::new_spanned(
            f,
            "You need to specify this interface field kind with either an #[interface] or a #[conn(..)] attribute",
        ))
    }
}

/* -------------------------------------------------------------------------- */

struct Interface {
    vis: Visibility,
    ident: Ident,
    generics: Generics,
    fields: Vec<InterfaceField>,
}

impl Parse for Interface {
    fn parse(buffer: syn::parse::ParseStream) -> syn::Result<Self> {
        let interface_struct = ItemStruct::parse(buffer)?;
        let ItemStruct { vis, ident, generics, fields, .. } =
            interface_struct.clone();

        let mut interface_fields = Vec::new();

        for f in fields {
            interface_fields.push(InterfaceField::try_from(f)?)
        }

        Ok(Interface { vis, ident, generics, fields: interface_fields })
    }
}

/* -------------------------------------------------------------------------- */

fn emit_solution(i: &Interface) -> TokenStream {
    fn solution_ty(f: InterfaceField) -> TokenStream {
        match f {
            InterfaceField::Connector(..) => {
                quote! { engi::unit::Quantity }
            }
            InterfaceField::Stateful { s, inner } => {
                quote! { engi::model::state::Stateful<Quantity, #s> }
            }
        }
    }

    fn solution_disassembly(f: InterfaceField) -> TokenStream {
        match f {
            InterfaceField::Connector(..) => quote! {
                // todo: shennaniganize
                let sol = assembled.connectors[cursor.connector].clone();
                cursor.connector += 1;
                sol
            },
            InterfaceField::Stateful { s, inner } => quote! {
                engi::model::state::Stateful::from_fn(|_| { #{ solution_disassembly(*inner.clone()) } })
            },
        }
    }
    let solution_ident = &suffix_ident(&i.ident, "Solution");

    quote! {
        #[derive(Clone, Default)]
        #{ &i.vis } struct #solution_ident {
            #(for f in &i.fields) {
                #{&f.vis()} #{&f.ident()}: #{&solution_ty(f.clone())},
            }
        }

        impl engi::model::InterfaceSolution for #solution_ident {
            fn disassemble(mut assembled: engi::model::solve::AssembledInterfaceSolution) -> Self {
                let mut cursor = engi::model::InterfaceCursor::default();

                Self {
                    #(for f in &i.fields) {
                        #{&f.ident()}: { #{&solution_disassembly(f.clone())} },
                    }
                }
            }
        }
    }
}

fn emit_view(i: &Interface) -> TokenStream {
    let view_ident = suffix_ident(&i.ident, "StatefulView");

    fn view_ty(f: InterfaceField) -> TokenStream {
        match f {
            InterfaceField::Connector(f, _) => {
                quote! { engi::model::state::Stateful<#{&f.ty}, S> }
            }
            InterfaceField::Stateful { s, inner } => {
                let inner_ty = inner.ty();
                quote! { engi::model::state::Stateful<engi::model::state::Stateful<#inner_ty, #s>, S> }
            }
        }
    }

    quote! {
        #[derive(Clone)]
        #{ &i.vis } struct #view_ident<S>
        where S: engi::model::state::States,
            [Self; S::COUNT]: Sized, {
            #(for f in &i.fields) {
                #{ &f.vis() } #{ &f.ident() }: #{ &view_ty(f.clone()) },
            }
        }

        impl engi::model::state::Stateable for #{&i.ident} {
            type View<S: engi::model::state::States> = #view_ident<S> where [Self; S::COUNT]: Sized;
        }

        impl<S> engi::model::state::StatefulView<S> for #view_ident<S>
        where S: engi::model::state::States,
            [#{&i.ident}; S::COUNT]: Sized {
            type Of = #{ &i.ident };

            fn new(states: &[Self::Of]) -> Self {
                Self {
                    #(for f in &i.fields) {
                        #{&f.ident()}: engi::model::state::Stateful::new(::core::array::from_fn(|i| states[i].#{&f.ident()}.clone())),
                    }
                }
            }
        }
    }
}

fn emit_builder(i: &Interface) -> TokenStream {
    fn builder_ty(f: InterfaceField) -> TokenStream {
        match f {
            InterfaceField::Connector(f, _) => {
                quote! { engi::model::ConnectorBuilder<'s> }
            }
            InterfaceField::Stateful { s, inner } => {
                let ty = builder_ty(*inner.clone());
                quote! { engi::model::state::Stateful<#ty, #s> }
            }
        }
    }

    fn template_bind(f: InterfaceField) -> TokenStream {
        match f {
            InterfaceField::Connector(f, _) => quote! {
                if let Some(ref _v) = template.#{&f.ident} {
                    self.#{&f.ident}.bind(_v);
                }
            },
            InterfaceField::Stateful { s, inner } => {
                template_bind(*inner.clone())
            }
        }
    }

    let builder_ident = suffix_ident(&i.ident, "Builder");

    quote! {
        #{&i.vis} struct #builder_ident<'s> {
            id: engi::model::InterfaceId,
            system: &'s engi::model::System,
            #(for f in &i.fields) {
                #{&f.vis()} #{&f.ident()}: #{&builder_ty(f.clone())},
            }
        }

        impl<'s> engi::model::InterfaceBuilder for #builder_ident<'s> {
            type Builds = #{&i.ident};

            fn id(&self) -> &engi::model::InterfaceId {
                &self.id
            }

            fn system(&self) -> &System {
                self.system
            }

            fn bind(&self, template: &<#{&i.ident} as engi::model::Interface>::Template) {
                use engi::model::InterfaceBuilder;

                #(for f in &i.fields) {
                    { #{&template_bind(f.clone())} };
                }
            }
        }
    }
}

fn emit_template(i: &Interface) -> TokenStream {
    let template_ident = suffix_ident(&i.ident, "Template");

    fn template_ty(f: InterfaceField) -> TokenStream {
        match f {
            InterfaceField::Connector(f, _) => {
                quote! { Option<engi::unit::Quantity> }
            }
            InterfaceField::Stateful { s, inner } => {
                let ty = template_ty(*inner.clone());
                quote! { engi::model::state::Stateful<#ty, #s> }
            }
        }
    }

    quote! {
        #[derive(Default, Clone)]
        #{&i.vis} struct #template_ident {
            #(for f in &i.fields) {
                #{&f.vis()} #{&f.ident()}: #{&template_ty(f.clone())},
            }
        }

        impl engi::model::InterfaceTemplate for #template_ident {}
    }
}

fn emit_interface_impl(i: &Interface) -> TokenStream {
    let view_ident = suffix_ident(&i.ident, "StatefulView");
    let template_ident = suffix_ident(&i.ident, "Template");
    let builder_ident = suffix_ident(&i.ident, "Builder");
    let solution_ident = suffix_ident(&i.ident, "Solution");
    let (impl_generics, ty_generics, where_clause) =
        i.generics.split_for_impl();

    fn constructor(f: InterfaceField, suffix: TokenStream) -> TokenStream {
        match f {
            InterfaceField::Connector(f, conn_attr) => quote! {
                engi::model::Connector::new(
                    engi::model::Variable::new(engi::symbol::Symbol::builder()
                        .name(format!("{}.{}{}", name, stringify!(#{&f.ident}), #suffix).leak())
                        .shape(#{&conn_attr.shape})
                        .unit(#{&conn_attr.unit})
                        .desc(#{&conn_attr.desc})
                        .build()
                    ),
                    { #{&conn_attr.condition} }
                )
            },
            InterfaceField::Stateful { s, inner } => {
                let expr = constructor(
                    *inner.clone(),
                    quote! {format!("[{}]{}", s, #suffix)},
                );

                quote! {
                    engi::model::state::Stateful::from_fn(|s| {
                        #expr
                    })
                }
            }
        }
    }

    fn builder(f: InterfaceField) -> TokenStream {
        match f {
            InterfaceField::Connector(..) => quote! {
                let builder = engi::model::ConnectorBuilder::new(
                    engi::model::ConnectorId::new(id.clone(), cursor.connector),
                    system
                );
                cursor.connector += 1;
                builder
            },
            InterfaceField::Stateful { s, inner } => quote! {
                engi::model::state::Stateful::from_fn(|_| { #{ builder(*inner.clone()) } })
            },
        }
    }

    fn assemble(f: InterfaceField, access: TokenStream) -> TokenStream {
        match f {
            InterfaceField::Connector(..) => quote! {
                connectors.push(#access)
            },
            InterfaceField::Stateful { s, inner } => quote! {
                // TODO: we need to add level-specific s variable ident when recursing stateful, so it dosent get shadowed

                for s in #{&inner.ident()} {
                    #{&assemble(*inner.clone(), quote!{s})}
                }
            },
        }
    }

    quote! {
        impl #impl_generics engi::model::Interface for #{&i.ident} #ty_generics #where_clause {
            type Solution = #solution_ident;
            type Builder<'s> = #builder_ident<'s>;
            type Template = #template_ident;

            fn new(name: &str) -> Self {
                Self {
                    #(for f in &i.fields) {
                        #{&f.ident()}: { #{&constructor(f.clone(), quote!{""})} },
                    }
                }
            }

            fn builder<'s>(id: engi::model::InterfaceId, system: &'s engi::model::System) -> Self::Builder<'s> {
                let mut cursor = engi::model::InterfaceCursor::default();

                Self::Builder {
                    #(for f in &i.fields) {
                        #{&f.ident()}: { #{&builder(f.clone())} },
                    }
                    id, system,
                }
            }

            fn assemble(self) -> engi::model::AssembledInterface {
                let mut connectors = Vec::new();

                #(for f in &i.fields) {
                    { #{&assemble(f.clone(), quote!{ self.#{&f.ident()} })} };
                }

                engi::model::AssembledInterface {
                    connectors
                }
            }
        }
    }
}

pub fn derive_interface(
    input: proc_macro::TokenStream,
) -> Result<proc_macro::TokenStream, syn::Error> {
    let m = Interface::parse.parse(input)?;

    Ok(quote! {
        #{&emit_view(&m)}
        #{&emit_builder(&m)}
        #{&emit_solution(&m)}
        #{&emit_template(&m)}

        #{&emit_interface_impl(&m)}
    }
    .into())
}
