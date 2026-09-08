pub fn to_superscript(num: i64) -> String {
    let superscripts = ["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"];

    num.to_string()
        .chars()
        .map(|c| {
            if let Some(digit) = c.to_digit(10) {
                superscripts[digit as usize]
            } else {
                "⁻"
            }
        })
        .collect()
}

#[crabtime::function]
fn impl_as_variant(input: TokenStream) {
    use proc_macro2::*;
    use quote::ToTokens;
    use syn::{
        Ident, Result, Token, Type, bracketed,
        parse::{Parse, ParseStream, *},
        *,
    };

    struct Args {
        ty: Type,
        conversions: Vec<(Ident, Type)>,
    }

    impl Parse for Args {
        fn parse(input: ParseStream) -> Result<Self> {
            let ty: Type = input.parse()?;

            input.parse::<Token![,]>()?;

            let content;
            bracketed!(content in input);

            let mut conversions = Vec::new();

            while !content.is_empty() {
                let from: Ident = content.parse()?;
                content.parse::<Token![=>]>()?;
                let to: Type = content.parse()?;

                conversions.push((from, to));

                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
            }

            Ok(Self { ty, conversions })
        }
    }

    let args: Args = parse2(input).unwrap();
    let ty = args.ty.to_token_stream().to_string();

    for (variant, output) in args.conversions {
        let output = output.to_token_stream().to_string();
        let variant_lower = variant.to_string().to_lowercase();

        crabtime::output! {
            impl {{ty}} {
                pub fn as_{{variant_lower}}(&self) -> Option<&{{output}}> {
                    match self {
                        {{ty}}::{{variant}}(out) => Some(out),
                        _ => None
                    }
                }

                pub fn into_{{variant_lower}}(self) -> Option<{{output}}> {
                    match self {
                        {{ty}}::{{variant}}(out) => Some(out),
                        _ => None
                    }
                }
            }
        };
    }
}

#[crabtime::function]
fn impl_op_permutations(input: TokenStream) {
    use std::collections::HashMap;

    use itertools::Itertools;
    use proc_macro2::*;
    use quote::ToTokens;
    use syn::{Token, parse::*, punctuated::Punctuated, *};

    #[derive(Default)]
    pub struct Args {
        pub types: Vec<String>,
        pub exclude_permutations: Vec<String>,
        pub exclude_specific: Vec<(String, String)>,
        pub op_exclusions: HashMap<String, (Vec<String>, Vec<String>)>,
        pub output: String,
        pub bodies: HashMap<String, String>,
    }

    impl Args {
        pub fn parse_meta(
            &mut self,
            meta: syn::meta::ParseNestedMeta,
        ) -> syn::Result<()> {
            if meta.path.is_ident("types") {
                let content;
                let value = meta.value()?;
                bracketed!(content in value);

                let types =
                    Punctuated::<Type, Token![,]>::parse_terminated(&content)?;

                self.types = types
                    .into_iter()
                    .map(|x| x.to_token_stream().to_string())
                    .collect();

                Ok(())
            } else if meta.path.is_ident("exclude_permutations") {
                let content;
                let value = meta.value()?;
                bracketed!(content in value);

                let types =
                    Punctuated::<Type, Token![,]>::parse_terminated(&content)?;

                self.exclude_permutations = types
                    .into_iter()
                    .map(|x| x.to_token_stream().to_string())
                    .collect();

                Ok(())
            } else if meta.path.is_ident("exclude_specific") {
                let content;
                let value = meta.value()?;
                bracketed!(content in value);

                let pairs =
                    Punctuated::<Expr, Token![,]>::parse_terminated(&content)?;

                for pair in pairs {
                    let Expr::Tuple(tuple) = pair else {
                        return Err(syn::Error::new_spanned(
                            pair,
                            "expected a 2-tuple, e.g. `(Foo, Bar)`",
                        ));
                    };

                    if tuple.elems.len() != 2 {
                        return Err(syn::Error::new_spanned(
                            tuple,
                            "expected a 2-tuple, e.g. `(Foo, Bar)`",
                        ));
                    }

                    let mut elems = tuple.elems.into_iter();

                    let a = elems.next().unwrap();
                    let b = elems.next().unwrap();

                    // Make sure both elements are types.
                    let a: Type = syn::parse2(a.to_token_stream())?;
                    let b: Type = syn::parse2(b.to_token_stream())?;

                    self.exclude_specific.push((
                        a.to_token_stream().to_string(),
                        b.to_token_stream().to_string(),
                    ));
                }

                Ok(())
            } else if meta.path.is_ident("exclude") {
                let content;
                let value = meta.value()?;
                braced!(content in value);

                while !content.is_empty() {
                    let op_ident: Ident = content.parse()?;
                    let _eq: Token![=] = content.parse()?;
                    let op_content;
                    braced!(op_content in content);

                    let mut lhs_types = Vec::new();
                    let mut rhs_types = Vec::new();

                    while !op_content.is_empty() {
                        let side_ident: Ident = op_content.parse()?;
                        let _eq2: Token![=] = op_content.parse()?;
                        let array_content;
                        bracketed!(array_content in op_content);

                        let types =
                            Punctuated::<Type, Token![,]>::parse_terminated(
                                &array_content,
                            )?;
                        let parsed_types: Vec<String> = types
                            .into_iter()
                            .map(|x| x.to_token_stream().to_string())
                            .collect();

                        if side_ident == "lhs" {
                            lhs_types = parsed_types;
                        } else if side_ident == "rhs" {
                            rhs_types = parsed_types;
                        } else {
                            return Err(syn::Error::new(
                                side_ident.span(),
                                "expected `lhs` or `rhs`",
                            ));
                        }

                        if op_content.peek(Token![,]) {
                            let _comma: Token![,] = op_content.parse()?;
                        }
                    }

                    self.op_exclusions
                        .insert(op_ident.to_string(), (lhs_types, rhs_types));

                    if content.peek(Token![,]) {
                        let _comma: Token![,] = content.parse()?;
                    }
                }
                Ok(())
            } else if meta.path.is_ident("out") {
                let value = meta.value()?;

                self.output =
                    value.parse::<Type>()?.to_token_stream().to_string();

                Ok(())
            } else {
                let name = meta
                    .path
                    .get_ident()
                    .ok_or_else(|| meta.error("expected operation name"))?
                    .to_string();

                let value = meta.value()?;
                let expr: Expr = value.parse()?;

                self.bodies.insert(name, expr.to_token_stream().to_string());

                Ok(())
            }
        }
    }

    let mut args = Args::default();

    let meta_parser = syn::meta::parser(|meta| args.parse_meta(meta));

    meta_parser
        .parse2(input)
        .expect("failed to parse impl_op_permutations arguments");

    let out = args.output;

    let is_excluded_specific = |a: &str, b: &str| {
        args.exclude_specific
            .iter()
            .any(|(x, y)| (a == x && b == y) || (a == y && b == x))
    };

    let is_op_excluded = |op: &str, a: &str, b: &str| -> bool {
        if let Some((lhs_excl, rhs_excl)) = args.op_exclusions.get(op) {
            if lhs_excl.iter().any(|x| x == a)
                || rhs_excl.iter().any(|x| x == b)
            {
                return true;
            }
        }
        false
    };

    for (a, b) in args.types.iter().cartesian_product(args.types.iter()) {
        if args.exclude_permutations.contains(a)
            && args.exclude_permutations.contains(b)
        {
            continue;
        }

        if is_excluded_specific(a, b) {
            continue;
        }

        if *a == out {
            if args.bodies.get("add").is_some() && !is_op_excluded("add", a, b)
            {
                crabtime::output! {
                    impl std::ops::AddAssign<{{b}}> for {{a}} {
                        fn add_assign(&mut self, rhs: {{b}}) {
                            *self = self.clone() + {{out}}::from(rhs)
                        }
                    }
                }
            }

            if args.bodies.get("sub").is_some() && !is_op_excluded("sub", a, b)
            {
                crabtime::output! {
                    impl std::ops::SubAssign<{{b}}> for {{a}} {
                        fn sub_assign(&mut self, rhs: {{b}}) {
                            *self = self.clone() - {{out}}::from(rhs)
                        }
                    }
                }
            }

            if args.bodies.get("mul").is_some() && !is_op_excluded("mul", a, b)
            {
                crabtime::output! {
                    impl std::ops::MulAssign<{{b}}> for {{a}} {
                        fn mul_assign(&mut self, rhs: {{b}}) {
                            *self = self.clone() * {{out}}::from(rhs)
                        }
                    }
                }
            }

            if args.bodies.get("div").is_some() && !is_op_excluded("div", a, b)
            {
                crabtime::output! {
                    impl std::ops::DivAssign<{{b}}> for {{a}} {
                        fn div_assign(&mut self, rhs: {{b}}) {
                            *self = self.clone() / {{out}}::from(rhs)
                        }
                    }
                }
            }
        }

        if let Some(partial_eq) = args.bodies.get("partial_eq")
            && (a.strip_prefix('&').unwrap_or(a).trim() == &out
                || b.strip_prefix('&').unwrap_or(b).trim() == &out)
            && !(a == b)
            && !(a.starts_with("&") && b.starts_with("&"))
            && !is_op_excluded("partial_eq", a, b)
        {
            crabtime::output! {
                impl std::cmp::PartialEq<{{b}}> for {{a}} {
                    fn eq(&self, rhs: &{{b}}) -> bool {
                        let lhs = {{out}}::from(self.clone());
                        let rhs = {{out}}::from(rhs.clone());

                        {{partial_eq}}
                    }
                }
            }
        }

        if let Some(add) = args.bodies.get("add") {
            if !is_op_excluded("add", a, b) {
                crabtime::output! {
                    impl std::ops::Add<{{b}}> for {{a}} {
                        type Output = {{out}};

                        fn add(self, rhs: {{b}}) -> {{out}} {
                            let lhs = {{out}}::from(self);
                            let rhs = {{out}}::from(rhs);

                            {{add}}
                        }
                    }
                }
            }
        }

        if let Some(mul) = args.bodies.get("mul") {
            if !is_op_excluded("mul", a, b) {
                crabtime::output! {
                    impl std::ops::Mul<{{b}}> for {{a}} {
                        type Output = {{out}};

                        fn mul(self, rhs: {{b}}) -> {{out}} {
                            let lhs = {{out}}::from(self);
                            let rhs = {{out}}::from(rhs);

                            {{mul}}
                        }
                    }
                }
            }
        }

        if let Some(div) = args.bodies.get("div") {
            if !is_op_excluded("div", a, b) {
                crabtime::output! {
                    impl std::ops::Div<{{b}}> for {{a}} {
                        type Output = {{out}};

                        fn div(self, rhs: {{b}}) -> {{out}} {
                            let lhs = {{out}}::from(self);
                            let rhs = {{out}}::from(rhs);

                            {{div}}
                        }
                    }
                }
            }
        }

        if let Some(sub) = args.bodies.get("sub") {
            if !is_op_excluded("sub", a, b) {
                crabtime::output! {
                    impl std::ops::Sub<{{b}}> for {{a}} {
                        type Output = {{out}};

                        fn sub(self, rhs: {{b}}) -> {{out}} {
                            let lhs = {{out}}::from(self);
                            let rhs = {{out}}::from(rhs);

                            {{sub}}
                        }
                    }
                }
            }
        }

        if let Some(pow) = args.bodies.get("pow") {
            if !is_op_excluded("pow", a, b) {
                crabtime::output! {
                    impl num::pow::Pow<{{b}}> for {{a}} {
                        type Output = {{out}};

                        fn pow(self, rhs: {{b}}) -> {{out}} {
                            let lhs = {{out}}::from(self);
                            let rhs = {{out}}::from(rhs);

                            {{pow}}
                        }
                    }
                }
            }
        }
    }
}

use std::sync::Arc;

pub(crate) use impl_as_variant;
pub(crate) use impl_op_permutations;

/* --------------------------------- TRAITS --------------------------------- */

pub(crate) trait ArcExt<T: Clone> {
    fn make_owned(&self) -> T;
}

impl<T: Clone> ArcExt<T> for Arc<T> {
    fn make_owned(&self) -> T {
        match Arc::try_unwrap(self.clone()) {
            Ok(value) => value,
            Err(arc) => (*arc).clone(),
        }
    }
}
