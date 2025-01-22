use syn::{
    fold::Fold,
    spanned::Spanned,
    token::{Colon, Gt, Lt, PathSep},
    AngleBracketedGenericArguments, FnArg, GenericArgument, Ident, Pat, PatIdent, PatType, Path,
    PathArguments, PathSegment, Receiver, Type, TypePath, TypeReference,
};

pub struct ChangeSelfToContext {
    pub local_idents: Box<[(Ident, bool)]>,
    pub replaced: bool,
}

impl Fold for ChangeSelfToContext {
    fn fold_ident(&mut self, i: proc_macro2::Ident) -> proc_macro2::Ident {
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
                if let Some((_, has_generics)) = self.local_idents.iter().find(|e| e.0 == seg.ident)
                {
                    let span = seg.ident.span();
                    i.segments = i.segments.into_iter().skip(1).collect();
                    if *has_generics {
                        i.segments[0].arguments = match i.segments[0].arguments.clone() {
                            PathArguments::AngleBracketed(mut args) => {
                                args.args = [GenericArgument::Type(Type::Path(TypePath {
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
                                }))]
                                .into_iter()
                                .chain(args.args)
                                .collect();
                                PathArguments::AngleBracketed(args)
                            }
                            _ => PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                                colon2_token: Some(PathSep::default()),
                                lt_token: Lt::default(),
                                args: [GenericArgument::Type(Type::Path(TypePath {
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
                                }))]
                                .into_iter()
                                .collect(),
                                gt_token: Gt::default(),
                            }),
                        }
                    }
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
                FnArg::Typed(PatType {
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
                })
            }
            FnArg::Typed(t) => FnArg::Typed(self.fold_pat_type(t)),
        }
    }
}
