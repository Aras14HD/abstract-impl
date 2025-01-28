use std::collections::{HashMap, HashSet};

use proc_macro2::Span;
use syn::{
    fold::Fold,
    spanned::Spanned,
    token::{Colon, Gt, Lt, Mut, PathSep, SelfValue},
    AngleBracketedGenericArguments, FnArg, GenericArgument, Ident, Pat, PatIdent, PatType, Path,
    PathArguments, PathSegment, Receiver, Type, TypePath, TypeReference,
};

#[derive(Debug, Clone)]
pub struct ChangeSelfToContext {
    pub local_idents: HashMap<Ident, (bool, Vec<Ident>)>,
    pub replaced: bool,
    pub found_idents: HashSet<Ident>,
}

impl Fold for ChangeSelfToContext {
    fn fold_ident(&mut self, i: proc_macro2::Ident) -> proc_macro2::Ident {
        self.found_idents.insert(i.clone());
        match i.to_string().as_str() {
            "Self" => {
                self.replaced = true;
                Ident::new("Context", i.span())
            }
            "self" => {
                self.replaced = true;
                Ident::new("context", i.span())
            }
            _ => i,
        }
    }
    fn fold_path(&mut self, mut i: syn::Path) -> syn::Path {
        if i.segments
            .first()
            .map(|seg| seg.ident.to_string() == "Self")
            .unwrap_or(false)
        {
            self.replaced = true;
            if let Some(seg) = i.segments.get(1) {
                if let Some((has_context, other_generics)) = self.local_idents.get(&seg.ident) {
                    let span = seg.ident.span();
                    i.segments = i.segments.into_iter().skip(1).collect();
                    i.segments[0].arguments = prepend_generics(
                        i.segments[0].arguments.clone(),
                        *has_context,
                        other_generics,
                        span,
                    );
                }
            }
        }
        i.segments = i
            .segments
            .into_iter()
            .map(|seg| self.fold_path_segment(seg))
            .collect();
        i
    }
    fn fold_fn_arg(&mut self, i: syn::FnArg) -> syn::FnArg {
        match i {
            FnArg::Receiver(Receiver {
                attrs,
                reference,
                mutability,
                self_token,
                ..
            }) => {
                self.replaced = true;
                FnArg::Typed(replace_reciever(attrs, reference, mutability, self_token))
            }
            FnArg::Typed(t) => FnArg::Typed(self.fold_pat_type(t)),
        }
    }
}
pub fn prepend_generics(
    arguments: PathArguments,
    has_context: bool,
    other_generics: &Vec<Ident>,
    span: Span,
) -> PathArguments {
    match arguments {
        PathArguments::AngleBracketed(mut args) => {
            args.args = has_context
                .then_some([GenericArgument::Type(Type::Path(TypePath {
                    qself: None,
                    path: Path {
                        leading_colon: None,
                        segments: [PathSegment {
                            ident: Ident::new("Context", span),
                            arguments: PathArguments::None,
                        }]
                        .into_iter()
                        .collect(),
                    },
                }))])
                .into_iter()
                .flatten()
                .chain(other_generics.into_iter().map(|gen| {
                    GenericArgument::Type(Type::Path(TypePath {
                        qself: None,
                        path: Path::from(gen.clone()),
                    }))
                }))
                .chain(args.args)
                .collect();
            PathArguments::AngleBracketed(args)
        }
        _ => PathArguments::AngleBracketed(AngleBracketedGenericArguments {
            colon2_token: Some(PathSep::default()),
            lt_token: Lt::default(),
            args: has_context
                .then_some([GenericArgument::Type(Type::Path(TypePath {
                    qself: None,
                    path: Path {
                        leading_colon: None,
                        segments: [PathSegment {
                            ident: Ident::new("Context", span),
                            arguments: PathArguments::None,
                        }]
                        .into_iter()
                        .collect(),
                    },
                }))])
                .into_iter()
                .flatten()
                .chain(other_generics.into_iter().map(|gen| {
                    GenericArgument::Type(Type::Path(TypePath {
                        qself: None,
                        path: Path::from(gen.clone()),
                    }))
                }))
                .collect(),
            gt_token: Gt::default(),
        }),
    }
}
fn replace_reciever(
    attrs: Vec<syn::Attribute>,
    reference: Option<(syn::token::And, Option<syn::Lifetime>)>,
    mutability: Option<Mut>,
    self_token: SelfValue,
) -> PatType {
    PatType {
        attrs,
        pat: Box::new(Pat::Ident(PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability,
            ident: Ident::new("context", self_token.span()),
            subpat: None,
        })),
        colon_token: Colon::default(),
        ty: Box::new(match reference {
            Some((and, lifetime)) => Type::Reference(TypeReference {
                and_token: and,
                lifetime,
                mutability,
                elem: Box::new(Type::Path(TypePath {
                    qself: None,
                    path: Path {
                        leading_colon: None,
                        segments: [PathSegment {
                            ident: Ident::new("Context", self_token.span()),
                            arguments: syn::PathArguments::None,
                        }]
                        .into_iter()
                        .collect(),
                    },
                })),
            }),
            None => Type::Path(TypePath {
                qself: None,
                path: Path {
                    leading_colon: None,
                    segments: [PathSegment {
                        ident: Ident::new("Context", self_token.span()),
                        arguments: syn::PathArguments::None,
                    }]
                    .into_iter()
                    .collect(),
                },
            }),
        }),
    }
}
