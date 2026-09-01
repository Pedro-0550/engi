pub fn to_superscript(num: i32) -> String {
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
                pub fn as_{{variant_lower}}(self) -> Option<{{output}}> {
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

    let body = |name: &str| {
        args.bodies
            .get(name)
            .unwrap_or_else(|| panic!("missing `{name}` operation body"))
    };

    let out = args.output;

    let add = body("add");
    let mul = body("mul");
    let div = body("div");
    let sub = body("sub");
    let pow = body("pow");
    let partial_eq = body("partial_eq");

    // `exclude_specific` is order-independent:
    //
    //     (A, B)
    //
    // excludes both:
    //
    //     A op B
    //     B op A
    //
    let is_excluded_specific = |a: &str, b: &str| {
        args.exclude_specific
            .iter()
            .any(|(x, y)| (a == x && b == y) || (a == y && b == x))
    };

    for (a, b) in args.types.iter().cartesian_product(args.types.iter()) {
        // `exclude_permutations = [A, B, C]` means that no permutation where
        // BOTH operands belong to that set is generated.
        //
        // Excludes:
        //     A-A, A-B, A-C
        //     B-A, B-B, B-C
        //     C-A, C-B, C-C
        //
        // But does NOT exclude:
        //     A-X, X-A
        //     B-X, X-B
        //     C-X, X-C
        //
        // where X is not in `exclude_permutations`.
        if args.exclude_permutations.contains(a)
            && args.exclude_permutations.contains(b)
        {
            continue;
        }

        // Explicit pair exclusions are also order-independent.
        if is_excluded_specific(a, b) {
            continue;
        }

        if *a == out {
            crabtime::output! {
                impl std::ops::AddAssign<{{b}}> for {{a}} {
                    fn add_assign(&mut self, rhs: {{b}}) {
                        *self = self.clone() + {{out}}::from(rhs)
                    }
                }

                impl std::ops::SubAssign<{{b}}> for {{a}} {
                    fn sub_assign(&mut self, rhs: {{b}}) {
                        *self = self.clone() - {{out}}::from(rhs)
                    }
                }

                impl std::ops::MulAssign<{{b}}> for {{a}} {
                    fn mul_assign(&mut self, rhs: {{b}}) {
                        *self = self.clone() * {{out}}::from(rhs)
                    }
                }

                impl std::ops::DivAssign<{{b}}> for {{a}} {
                    fn div_assign(&mut self, rhs: {{b}}) {
                        *self = self.clone() / {{out}}::from(rhs)
                    }
                }
            }
        }

        if (a == &out || b == &out) && !(a == b) {
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

        crabtime::output! {
            impl std::ops::Add<{{b}}> for {{a}} {
                type Output = {{out}};

                fn add(self, rhs: {{b}}) -> {{out}} {
                    let lhs = {{out}}::from(self);
                    let rhs = {{out}}::from(rhs);

                    {{add}}
                }
            }

            impl std::ops::Mul<{{b}}> for {{a}} {
                type Output = {{out}};

                fn mul(self, rhs: {{b}}) -> {{out}} {
                    let lhs = {{out}}::from(self);
                    let rhs = {{out}}::from(rhs);

                    {{mul}}
                }
            }

            impl std::ops::Div<{{b}}> for {{a}} {
                type Output = {{out}};

                fn div(self, rhs: {{b}}) -> {{out}} {
                    let lhs = {{out}}::from(self);
                    let rhs = {{out}}::from(rhs);

                    {{div}}
                }
            }

            impl std::ops::Sub<{{b}}> for {{a}} {
                type Output = {{out}};

                fn sub(self, rhs: {{b}}) -> {{out}} {
                    let lhs = {{out}}::from(self);
                    let rhs = {{out}}::from(rhs);

                    {{sub}}
                }
            }

            impl num::pow::Pow<{{b}}> for {{a}} {
                type Output = {{out}};

                fn pow(self, rhs: {{b}}) -> {{out}} {
                    let lhs = {{out}}::from(self);
                    let rhs = {{out}}::from(rhs);

                    {{pow}}
                }
            }
        };
    }
}

pub(crate) use impl_as_variant;
pub(crate) use impl_op_permutations;
