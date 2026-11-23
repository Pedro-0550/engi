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

// this macro is mostly AI generated.
// I dont really care because its just clerical work which i cant be bothered to do or require quality.
// As terrible as it is, this is one of those things where it works and you just dont touch it
// include = [ (A, B), bi(A, B), ... ]
//     same pairs for every operation
// include = { pow = [ (A, B), ... ], _ = [ bi(A, B), ... ] }
//     per-operation pairs; `_` is the fallback for operations without their own
//     list (so `pow` above does NOT also get the `_` pairs)
// Entries and `name = [...]` items must be separated by commas (trailing comma ok).
//   (A, B)    -> only `A op B`
//   bi(A, B)  -> `A op B` and `B op A`
// Patterns:
//   T         -> exactly T
//   &? T      -> T and &T
//   *         -> any type in `types`
//   * except [X, Y] -> any type in `types` except X and Y (X, Y may use `&?` and `*`)
#[crabtime::function]
fn impl_op_permutations(input: TokenStream) {
    use std::collections::{HashMap, HashSet};

    use itertools::Itertools;
    use proc_macro2::*;
    use quote::ToTokens;
    use syn::{Token, parse::*, punctuated::Punctuated, *};

    // One `(A, B)` / `bi(A, B)` entry of `include = [...]`.
    // The sides stay as raw tokens because they may contain `*` and `&?`,
    // which aren't valid Rust types.
    pub struct IncludeEntry {
        pub bi: bool,
        pub lhs: TokenStream,
        pub rhs: TokenStream,
    }

    // Parsed `include`: `_` fallback list plus per-operation lists.
    #[derive(Default)]
    pub struct IncludeSpec {
        pub default: Option<Vec<IncludeEntry>>,
        pub per_op: HashMap<String, Vec<IncludeEntry>>,
    }

    #[derive(Default)]
    pub struct Args {
        /// (name, full text), e.g. ("S", "S : Clone")
        pub params: Vec<(String, String)>,
        pub where_preds: Vec<(String, TokenStream)>,
        pub types: Vec<String>,
        /// Required: the (a, b) pairs to implement, optionally per operation.
        pub include: Option<IncludeSpec>,
        pub convert_into: String,
        pub output: String,
        pub bodies: HashMap<String, String>,
    }

    fn err(msg: &str) -> syn::Error {
        syn::Error::new(proc_macro2::Span::call_site(), msg)
    }

    fn lex(s: &str) -> syn::Result<TokenStream> {
        s.parse::<TokenStream>()
            .map_err(|e| err(&format!("failed to lex `{}`: {}", s, e)))
    }

    // Splits on commas that are not nested inside `()`/`[]`/`{}` (those are
    // already token groups) or inside `<...>` (which are NOT groups).
    fn split_top(ts: TokenStream) -> Vec<Vec<TokenTree>> {
        let mut parts: Vec<Vec<TokenTree>> = Vec::new();
        let mut cur: Vec<TokenTree> = Vec::new();
        let mut depth = 0i32;

        for tt in ts {
            let mut is_comma = false;

            if let TokenTree::Punct(p) = &tt {
                match p.as_char() {
                    '<' => depth += 1,
                    '>' => {
                        // ignore the `>` of `->` and `=>`
                        let arrow = matches!(
                            cur.last(),
                            Some(TokenTree::Punct(q))
                                if (q.as_char() == '-' || q.as_char() == '=')
                                    && q.spacing() == Spacing::Joint
                        );
                        if !arrow {
                            depth -= 1;
                        }
                    }
                    ',' if depth <= 0 => is_comma = true,
                    _ => {}
                }
            }

            if is_comma {
                parts.push(std::mem::take(&mut cur));
            } else {
                cur.push(tt);
            }
        }

        if !cur.is_empty() {
            parts.push(cur);
        }

        parts.retain(|p| !p.is_empty());
        parts
    }

    // Parses a list of `(A, B)` / `bi(A, B)` entries. Commas between entries
    // are required (a trailing comma is fine).
    fn parse_entries(ts: TokenStream) -> syn::Result<Vec<IncludeEntry>> {
        let mut entries = Vec::new();
        let mut it = ts.into_iter();

        while let Some(tt) = it.next() {
            let (bi, group) = match tt {
                TokenTree::Ident(i) if i == "bi" => match it.next() {
                    Some(TokenTree::Group(g))
                        if g.delimiter() == Delimiter::Parenthesis =>
                    {
                        (true, g)
                    }
                    _ => {
                        return Err(err(
                            "include: expected `(A, B)` after `bi`",
                        ));
                    }
                },
                TokenTree::Group(g)
                    if g.delimiter() == Delimiter::Parenthesis =>
                {
                    (false, g)
                }
                _ => {
                    return Err(err(
                        "include: expected `(A, B)` or `bi(A, B)`",
                    ));
                }
            };

            let mut sides = split_top(group.stream());
            if sides.len() != 2 {
                return Err(err("include: expected exactly two patterns"));
            }

            let rhs = sides.pop().unwrap().into_iter().collect::<TokenStream>();
            let lhs = sides.pop().unwrap().into_iter().collect::<TokenStream>();

            entries.push(IncludeEntry { bi, lhs, rhs });

            match it.next() {
                None => {}
                Some(TokenTree::Punct(p)) if p.as_char() == ',' => {}
                Some(_) => {
                    return Err(err("include: expected `,` between entries"));
                }
            }
        }

        Ok(entries)
    }

    // Parses the inside of `include = { pow = [...], _ = [...] }`.
    fn parse_include_map(
        ts: TokenStream,
        spec: &mut IncludeSpec,
    ) -> syn::Result<()> {
        let mut it = ts.into_iter();

        while let Some(tt) = it.next() {
            let name = match tt {
                TokenTree::Ident(i) => i.to_string(),
                _ => {
                    return Err(err(
                        "include: expected an operation name or `_`",
                    ));
                }
            };

            match it.next() {
                Some(TokenTree::Punct(p)) if p.as_char() == '=' => {}
                _ => return Err(err("include: expected `=` after the name")),
            }

            let list = match it.next() {
                Some(TokenTree::Group(g))
                    if g.delimiter() == Delimiter::Bracket =>
                {
                    parse_entries(g.stream())?
                }
                _ => return Err(err("include: expected `[...]` after `=`")),
            };

            if name == "_" {
                spec.default.get_or_insert_with(Vec::new).extend(list);
            } else {
                spec.per_op.entry(name).or_default().extend(list);
            }

            match it.next() {
                None => {}
                Some(TokenTree::Punct(p)) if p.as_char() == ',' => {}
                Some(_) => {
                    return Err(err("include: expected `,` after the list"));
                }
            }
        }

        Ok(())
    }

    // All (lhs, rhs) pairs described by a list of include entries, deduplicated.
    fn expand_entries(
        entries: &[IncludeEntry],
        types: &[(String, TokenStream)],
        type_set: &HashSet<String>,
    ) -> syn::Result<Vec<(String, String)>> {
        let mut seen: HashSet<(String, String)> = HashSet::new();
        let mut pairs: Vec<(String, String)> = Vec::new();

        for e in entries {
            let lhs = expand_pattern(e.lhs.clone(), types, type_set)?;
            let rhs = expand_pattern(e.rhs.clone(), types, type_set)?;

            for (pat, side) in [(&e.lhs, &lhs), (&e.rhs, &rhs)] {
                if side.is_empty() {
                    return Err(err(&format!(
                        "include: pattern `{}` matches none of the `types`",
                        pat
                    )));
                }
            }

            for (a, b) in lhs.iter().cartesian_product(rhs.iter()) {
                if seen.insert((a.clone(), b.clone())) {
                    pairs.push((a.clone(), b.clone()));
                }
                if e.bi && seen.insert((b.clone(), a.clone())) {
                    pairs.push((b.clone(), a.clone()));
                }
            }
        }

        Ok(pairs)
    }

    // Finds the first `*` wildcard (depth-first, left to right) and returns one
    // token stream per type it can stand for, or `None` if there is no wildcard.
    // `* except [A, B]` is a wildcard that skips A and B. Each exclusion is
    // itself a pattern, so `&? T` skips both `T` and `&T`, and `Stateful<*, S>`
    // skips every listed `Stateful<_, S>`. `*const` / `*mut` raw pointers are
    // ignored.
    fn expand_first(
        toks: &[TokenTree],
        types: &[(String, TokenStream)],
        type_set: &HashSet<String>,
    ) -> syn::Result<Option<Vec<TokenStream>>> {
        for i in 0..toks.len() {
            match &toks[i] {
                TokenTree::Punct(p)
                    if p.as_char() == '*'
                        && !matches!(
                            toks.get(i + 1),
                            Some(TokenTree::Ident(n)) if n == "const" || n == "mut"
                        ) =>
                {
                    let mut consumed = i + 1;
                    let mut excluded: HashSet<String> = HashSet::new();

                    // `* except [A, B, ...]`
                    if matches!(
                        toks.get(i + 1),
                        Some(TokenTree::Ident(k)) if k == "except"
                    ) {
                        match toks.get(i + 2) {
                            Some(TokenTree::Group(g))
                                if g.delimiter() == Delimiter::Bracket =>
                            {
                                consumed = i + 3;

                                for part in split_top(g.stream()) {
                                    excluded.extend(expand_pattern(
                                        part.into_iter()
                                            .collect::<TokenStream>(),
                                        types,
                                        type_set,
                                    )?);
                                }
                            }
                            _ => {
                                return Err(err(
                                    "include: expected `[...]` after `* except`",
                                ));
                            }
                        }
                    }

                    let prefix: TokenStream =
                        toks[..i].iter().cloned().collect();
                    let suffix: TokenStream =
                        toks[consumed..].iter().cloned().collect();

                    return Ok(Some(
                        types
                            .iter()
                            .filter(|(s, _)| !excluded.contains(s))
                            .map(|(_, t)| {
                                let mut o = prefix.clone();
                                o.extend(t.clone());
                                o.extend(suffix.clone());
                                o
                            })
                            .collect(),
                    ));
                }
                TokenTree::Group(g) => {
                    let inner: Vec<TokenTree> =
                        g.stream().into_iter().collect();

                    if let Some(alts) = expand_first(&inner, types, type_set)? {
                        let prefix: TokenStream =
                            toks[..i].iter().cloned().collect();
                        let suffix: TokenStream =
                            toks[i + 1..].iter().cloned().collect();

                        return Ok(Some(
                            alts.into_iter()
                                .map(|alt| {
                                    let mut ng = proc_macro2::Group::new(
                                        g.delimiter(),
                                        alt,
                                    );
                                    ng.set_span(g.span());

                                    let mut o = prefix.clone();
                                    o.extend([TokenTree::Group(ng)]);
                                    o.extend(suffix.clone());
                                    o
                                })
                                .collect(),
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(None)
    }

    // Every `*` independently ranges over the (non-excluded) types.
    fn expand_wildcards(
        ts: TokenStream,
        types: &[(String, TokenStream)],
        type_set: &HashSet<String>,
    ) -> syn::Result<Vec<TokenStream>> {
        let toks: Vec<TokenTree> = ts.clone().into_iter().collect();

        match expand_first(&toks, types, type_set)? {
            Some(alts) => {
                let mut out = Vec::new();
                for a in alts {
                    out.extend(expand_wildcards(a, types, type_set)?);
                }
                Ok(out)
            }
            None => Ok(vec![ts]),
        }
    }

    // Turns one side of an include entry into the concrete type strings it
    // stands for. Anything that isn't actually in `types` is dropped.
    fn expand_pattern(
        pat: TokenStream,
        types: &[(String, TokenStream)],
        type_set: &HashSet<String>,
    ) -> syn::Result<Vec<String>> {
        let toks: Vec<TokenTree> = pat.into_iter().collect();

        // leading `&?` => both `T` and `&T`
        let optional_ref = matches!(
            (toks.first(), toks.get(1)),
            (Some(TokenTree::Punct(a)), Some(TokenTree::Punct(b)))
                if a.as_char() == '&' && b.as_char() == '?'
        );

        let mut variants: Vec<TokenStream> = Vec::new();
        if optional_ref {
            let rest: Vec<TokenTree> = toks[2..].to_vec();
            variants.push(rest.iter().cloned().collect());

            let mut with_ref: Vec<TokenTree> =
                vec![TokenTree::Punct(Punct::new('&', Spacing::Alone))];
            with_ref.extend(rest);
            variants.push(with_ref.into_iter().collect());
        } else {
            variants.push(toks.into_iter().collect());
        }

        let mut out: Vec<String> = Vec::new();
        for v in variants {
            for e in expand_wildcards(v, types, type_set)? {
                if let Ok(ty) = syn::parse2::<Type>(e) {
                    let s = ty.to_token_stream().to_string();
                    if type_set.contains(&s) && !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }

        Ok(out)
    }

    impl Args {
        pub fn parse_meta(
            &mut self,
            meta: syn::meta::ParseNestedMeta,
        ) -> syn::Result<()> {
            if meta.path.is_ident("params") {
                let content;
                let value = meta.value()?;
                bracketed!(content in value);

                let params =
                    Punctuated::<GenericParam, Token![,]>::parse_terminated(
                        &content,
                    )?;

                self.params = params
                    .into_iter()
                    .map(|p| {
                        let name = match &p {
                            GenericParam::Type(t) => t.ident.to_string(),
                            GenericParam::Lifetime(l) => {
                                l.lifetime.ident.to_string()
                            }
                            GenericParam::Const(c) => c.ident.to_string(),
                        };
                        (name, p.to_token_stream().to_string())
                    })
                    .collect();

                Ok(())
            } else if meta.path.is_ident("where") {
                let content;
                let value = meta.value()?;
                bracketed!(content in value);

                let preds =
                    Punctuated::<WherePredicate, Token![,]>::parse_terminated(
                        &content,
                    )?;

                self.where_preds = preds
                    .into_iter()
                    .map(|x| {
                        (x.to_token_stream().to_string(), x.to_token_stream())
                    })
                    .collect();

                Ok(())
            } else if meta.path.is_ident("types") {
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
            } else if meta.path.is_ident("include") {
                let g: proc_macro2::Group = meta.value()?.parse()?;

                let mut spec = IncludeSpec::default();
                match g.delimiter() {
                    Delimiter::Bracket => {
                        spec.default = Some(parse_entries(g.stream())?);
                    }
                    Delimiter::Brace => {
                        parse_include_map(g.stream(), &mut spec)?;
                    }
                    _ => {
                        return Err(err(
                            "include: expected `[...]` or `{...}`",
                        ));
                    }
                }
                self.include = Some(spec);

                Ok(())
            } else if meta.path.is_ident("out") {
                let value = meta.value()?;

                self.output =
                    value.parse::<Type>()?.to_token_stream().to_string();

                Ok(())
            } else if meta.path.is_ident("into") {
                let value = meta.value()?;

                self.convert_into =
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

    // Everything that can fail lives in here so errors can use `?`.
    // This is a closure rather than a nested `fn` because `crabtime::output!`
    // writes to the local `__output_buffer__`, which a `fn` item can't capture.
    let mut generate = |input: TokenStream| -> syn::Result<()> {
        let mut args = Args::default();

        let meta_parser = syn::meta::parser(|meta| args.parse_meta(meta));

        meta_parser.parse2(input)?;

        let into = args.convert_into;
        let out = args.output;

        // Token-tree walk so `S` doesn't match `States` / `Stateful`.
        fn mentions(ts: TokenStream, name: &str) -> bool {
            ts.into_iter().any(|tt| match tt {
                TokenTree::Ident(i) => i == name,
                TokenTree::Group(g) => mentions(g.stream(), name),
                _ => false,
            })
        }

        let Some(spec) = &args.include else {
            return Err(err("impl_op_permutations: missing `include`"));
        };

        for op in spec.per_op.keys() {
            if !args.bodies.contains_key(op) {
                return Err(err(&format!(
                    "include: `{}` has no body in this invocation",
                    op
                )));
            }
        }

        let type_set: HashSet<String> = args.types.iter().cloned().collect();
        let type_entries: Vec<(String, TokenStream)> = args
            .types
            .iter()
            .map(|t| -> syn::Result<(String, TokenStream)> {
                Ok((t.clone(), lex(t)?))
            })
            .collect::<syn::Result<Vec<_>>>()?;

        let default_pairs: Option<Vec<(String, String)>> = spec
            .default
            .as_ref()
            .map(|e| expand_entries(e, &type_entries, &type_set))
            .transpose()?;

        // pairs per operation, plus the union of all of them (generation order)
        let mut pairs_by_op: HashMap<String, HashSet<(String, String)>> =
            HashMap::new();
        let mut all_pairs: Vec<(String, String)> = Vec::new();
        let mut all_seen: HashSet<(String, String)> = HashSet::new();

        for op in ["add", "sub", "mul", "div", "pow", "partial_eq"] {
            if !args.bodies.contains_key(op) {
                continue;
            }

            let list = match spec.per_op.get(op) {
                Some(entries) => {
                    expand_entries(entries, &type_entries, &type_set)?
                }
                None => match &default_pairs {
                    Some(d) => d.clone(),
                    None => {
                        return Err(err(&format!(
                            "include: no pairs for `{}`: add `{} = [...]` or a `_ = [...]` fallback",
                            op, op
                        )));
                    }
                },
            };

            for pair in &list {
                if all_seen.insert(pair.clone()) {
                    all_pairs.push(pair.clone());
                }
            }

            pairs_by_op.insert(op.to_string(), list.into_iter().collect());
        }

        let op_has = |op: &str, a: &str, b: &str| -> bool {
            pairs_by_op
                .get(op)
                .map_or(false, |s| s.contains(&(a.to_string(), b.to_string())))
        };

        for (a, b) in all_pairs.iter() {
            let a_ts = lex(a)?;
            let b_ts = lex(b)?;

            // Params that appear in at least one of the two types
            // (otherwise the impl would hit E0207).
            let used: Vec<&(String, String)> = args
                .params
                .iter()
                .filter(|(name, _)| {
                    mentions(a_ts.clone(), name) || mentions(b_ts.clone(), name)
                })
                .collect();

            let generics = if used.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    used.iter()
                        .map(|(_, full)| full.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };

            // Drop any predicate that mentions a param we didn't declare.
            let preds: Vec<&(String, TokenStream)> = args
                .where_preds
                .iter()
                .filter(|(_, ts)| {
                    args.params.iter().all(|(name, _)| {
                        used.iter().any(|(u, _)| u == name)
                            || !mentions(ts.clone(), name)
                    })
                })
                .collect();

            let where_clause = if preds.is_empty() {
                String::new()
            } else {
                format!(
                    "where {}",
                    preds
                        .iter()
                        .map(|(s, _)| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };

            // type + where clause as one string, so each impl header has a single
            // interpolation right before the body brace (same shape as the original).
            let target = format!("{a} {where_clause}");

            if *a == out {
                if args.bodies.get("add").is_some() && op_has("add", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::AddAssign<{{b}}> for {{target}} {
                            fn add_assign(&mut self, rhs: {{b}}) {
                                *self = self.clone() + <{{b}} as Into<{{into}}>>::into(rhs)
                            }
                        }
                    }
                }

                if args.bodies.get("sub").is_some() && op_has("sub", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::SubAssign<{{b}}> for {{target}} {
                            fn sub_assign(&mut self, rhs: {{b}}) {
                                *self = self.clone() - <{{b}} as Into<{{into}}>>::into(rhs)
                            }
                        }
                    }
                }

                if args.bodies.get("mul").is_some() && op_has("mul", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::MulAssign<{{b}}> for {{target}} {
                            fn mul_assign(&mut self, rhs: {{b}}) {
                                *self = self.clone() * <{{b}} as Into<{{into}}>>::into(rhs)
                            }
                        }
                    }
                }

                if args.bodies.get("div").is_some() && op_has("div", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::DivAssign<{{b}}> for {{target}} {
                            fn div_assign(&mut self, rhs: {{b}}) {
                                *self = self.clone() / <{{b}} as Into<{{into}}>>::into(rhs)
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
                && op_has("partial_eq", a, b)
            {
                crabtime::output! {
                    impl {{generics}} std::cmp::PartialEq<{{b}}> for {{target}} {
                        fn eq(&self, rhs: &{{b}}) -> bool {
                            let lhs = <{{a}} as Into<{{into}}>>::into(self.clone());
                            let rhs = <{{b}} as Into<{{into}}>>::into(rhs.clone());

                            {{partial_eq}}
                        }
                    }
                }
            }

            if let Some(add) = args.bodies.get("add") {
                if op_has("add", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::Add<{{b}}> for {{target}} {
                            type Output = {{out}};

                            fn add(self, rhs: {{b}}) -> {{out}} {
                                let lhs = <{{a}} as Into<{{into}}>>::into(self);
                                let rhs = <{{b}} as Into<{{into}}>>::into(rhs);

                                {{add}}
                            }
                        }
                    }
                }
            }

            if let Some(mul) = args.bodies.get("mul") {
                if op_has("mul", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::Mul<{{b}}> for {{target}} {
                            type Output = {{out}};

                            fn mul(self, rhs: {{b}}) -> {{out}} {
                                let lhs = <{{a}} as Into<{{into}}>>::into(self);
                                let rhs = <{{b}} as Into<{{into}}>>::into(rhs);

                                {{mul}}
                            }
                        }
                    }
                }
            }

            if let Some(div) = args.bodies.get("div") {
                if op_has("div", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::Div<{{b}}> for {{target}} {
                            type Output = {{out}};

                            fn div(self, rhs: {{b}}) -> {{out}} {
                                let lhs = <{{a}} as Into<{{into}}>>::into(self);
                                let rhs = <{{b}} as Into<{{into}}>>::into(rhs);

                                {{div}}
                            }
                        }
                    }
                }
            }

            if let Some(sub) = args.bodies.get("sub") {
                if op_has("sub", a, b) {
                    crabtime::output! {
                        impl {{generics}} std::ops::Sub<{{b}}> for {{target}} {
                            type Output = {{out}};

                            fn sub(self, rhs: {{b}}) -> {{out}} {
                                let lhs = <{{a}} as Into<{{into}}>>::into(self);
                                let rhs = <{{b}} as Into<{{into}}>>::into(rhs);

                                {{sub}}
                            }
                        }
                    }
                }
            }

            if let Some(pow) = args.bodies.get("pow") {
                if op_has("pow", a, b) {
                    crabtime::output! {
                        impl {{generics}} num::pow::Pow<{{b}}> for {{target}} {
                            type Output = {{out}};

                            fn pow(self, rhs: {{b}}) -> {{out}} {
                                let lhs = <{{a}} as Into<{{into}}>>::into(self);
                                let rhs = <{{b}} as Into<{{into}}>>::into(rhs);

                                {{pow}}
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    };

    if let Err(e) = generate(input) {
        let msg = e.to_compile_error().to_string();
        crabtime::output! {
            {{msg}}
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
