use std::collections::HashMap;
use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::{braced, parse_macro_input, token, Expr, LitInt, Token};

struct BitmaskMatch {
    _match: Token![match],
    match_var: Expr,
    _brace_token: token::Brace,
    arms: Punctuated<MatchArm, Token![,]>,
}

impl Parse for BitmaskMatch {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let preds;
        Ok(Self {
            _match: input.parse().expect("a"),
            match_var: Expr::parse_without_eager_brace(input)?,
            _brace_token: braced!(preds in input),
            arms: preds
                .parse_terminated(MatchArm::parse, Token![,])
                .expect("c"),
        })
    }
}

struct MatchArm {
    predicate: BitmaskMatchPredicate,
    _fat_arrow: Token![=>],
    body: Expr,
}

impl Parse for MatchArm {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(MatchArm {
            predicate: input.parse().expect("ma"),
            _fat_arrow: input.parse().expect("mb"),
            body: input.parse().expect("mc"),
        })
    }
}

enum BitmaskMatchPredicate {
    Exact(LitInt),
    Fallback(Token![_]),
    Complex(Ident),
}

impl Parse for BitmaskMatchPredicate {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(LitInt) {
            return Ok(Self::Exact(input.parse()?));
        }
        if input.peek(Token![_]) {
            return Ok(Self::Fallback(input.parse()?));
        }

        Ok(Self::Complex(input.parse()?))
    }
}

pub(crate) fn bitmaskeq(input: TokenStream) -> TokenStream {
    let BitmaskMatch { match_var, arms, .. } = parse_macro_input!(input as _);

    let mut body = quote!();
    for arm in arms {
        let armbody = &arm.body;
        match arm.predicate {
            BitmaskMatchPredicate::Exact(e) => {
                body = quote! {
                    #body
                    #e => #armbody,
                }
            }

            BitmaskMatchPredicate::Fallback(u) => {
                body = quote! {
                    #body
                    #u => #armbody,
                }
            }

            BitmaskMatchPredicate::Complex(pred) => {
                let pred = pred.to_string();
                if !pred.starts_with("m_") {
                    panic!("mask predicate must start with 'm_'");
                }

                let mut captures = HashMap::new();
                let mut mask = "0b".to_owned();
                let mut value = "0b".to_owned();
                let mut empty_mask = "0b".to_owned();

                for p in pred.chars().skip("m_".len()) {
                    #[rustfmt::skip]
                    let (maskc, valuec, emptyc, capture) = match p {
                        '0'             => ('1', '0', '0', None),
                        '1'             => ('1', '1', '0', None),
                        'x'             => ('0', '0', '0', None),
                        '_'             => ('_', '_', '_', None), // separater
                        cap @ 'a'..='z' => ('0', '0', '0', Some(cap)),
                        _ => panic!("invalid mask predicate {p}"),
                    };

                    if let Some(capture) = capture {
                        captures
                            .entry(capture)
                            .or_insert_with(|| empty_mask.clone());
                    }

                    for (k, v) in &mut captures {
                        v.push(match *k {
                            k if capture.map_or(false, |c| c == k) => '1',
                            _ if p == '_' => '_',
                            _ => '0',
                        });
                    }

                    mask.push(maskc);
                    value.push(valuec);
                    empty_mask.push(emptyc);
                }

                let mut captures_quote = quote!();
                for (k, v) in captures {
                    let k = TokenStream2::from_str(&format!("{k}")).unwrap();
                    let v = TokenStream2::from_str(&v).unwrap();
                    captures_quote = quote!(
                        #captures_quote
                        let #k = __i & #v;
                    )
                }

                let mask = TokenStream2::from_str(&mask).unwrap();
                let value = TokenStream2::from_str(&value).unwrap();
                body = quote! {
                    #body
                    __i if (__i & #mask) == #value => {
                        #captures_quote
                        #armbody
                    }
                }
            }
        }
    }

    quote! {
        match #match_var {
            #body
        }
    }
    .into()
}
