use core::panic;
use std::any::Any;

use crate::{dummy::generate_dummy_impl, mac::generate_impl_macro};

use super::change_self::ChangeSelfToContext;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    fold::Fold,
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Bracket, Colon, Comma, Const, Mod, Paren, Pound, Pub, Where},
    AttrStyle, Attribute, GenericArgument, GenericParam, Generics, Ident, ImplItem, ImplItemConst,
    ImplItemFn, ImplItemType, Item, ItemConst, ItemFn, ItemImpl, ItemMod, ItemType, MetaList, Path,
    PathArguments, ReturnType, Type, TypeParam, Visibility, WhereClause,
};

pub fn transform(imp: ItemImpl, use_dummy: bool, use_macro: bool, legacy_order: bool) -> ItemMod {
    let copy = imp.clone();
    let ItemImpl {
        mut attrs,
        unsafety,
        generics,
        self_ty,
        items,
        trait_,
        ..
    } = imp;
    let local_idents = items
        .iter()
        .filter_map(|e| match e {
            ImplItem::Type(ty) => Some((ty.ident.clone(), true)),
            ImplItem::Const(c) => Some((c.ident.clone(), true)),
            ImplItem::Fn(f) => Some((f.sig.ident.clone(), true)),
            _ => None,
        })
        .collect::<Box<_>>();
    let mut folder = ChangeSelfToContext {
        local_idents,
        replaced: false,
    };

    let trait_ = trait_.expect("Impl names are neccesary").1;
    let Type::Path(syn::TypePath {
        qself: None,
        path: ty,
    }) = *self_ty
    else {
        panic!("Type name has to be a Path")
    };
    #[cfg(feature = "impl_name_first")]
    let (ty, trait_) = if !legacy_order {
        (trait_, ty)
    } else {
        (ty, trait_)
    };
    assert!(ty.segments.len() == 1, "Impl names have to be Idents");
    let ty_generics = match ty.segments[0].arguments.clone() {
        PathArguments::None => Punctuated::new(),
        PathArguments::AngleBracketed(args) => args.args.into(),
        PathArguments::Parenthesized(_) => panic!("Impls are not functions"),
    };
    let ty = ty.segments[0].ident.clone();

    let mut processed: Vec<Item> = items
        .into_iter()
        .map(|item| match item {
            ImplItem::Const(c) => {
                process_const(c, generics.clone(), ty_generics.clone(), &mut folder)
            }
            ImplItem::Fn(f) => process_fn(f, generics.clone(), ty_generics.clone(), &mut folder),
            ImplItem::Type(t) => {
                process_type(t, generics.clone(), ty_generics.clone(), &mut folder)
            }
            _ => panic!("abstract impl can only contain functions/methods and types!"),
        })
        .collect();
    processed.push(parse_quote! {use super::*;});

    attrs.push(Attribute {
        pound_token: Pound::default(),
        style: AttrStyle::Outer,
        bracket_token: Bracket::default(),
        meta: syn::Meta::List(MetaList {
            path: Path::from(Ident::new("allow", Span::call_site())),
            delimiter: syn::MacroDelimiter::Paren(Paren::default()),
            tokens: quote!(non_snake_case, type_alias_bounds),
        }),
    });

    // Dummy Impl (for errors)
    #[cfg(feature = "dummy")]
    if use_dummy {
        processed.push(parse_quote! {struct Dummy;});
        processed.push(generate_dummy_impl(
            copy.clone(),
            trait_.clone(),
            ty_generics.clone(),
        ));
    }
    #[cfg(feature = "macro")]
    if use_macro {
        processed.push(generate_impl_macro(
            copy,
            &ty,
            &mut folder,
            trait_,
            generics.clone(),
            ty_generics.clone(),
        ));
    }

    ItemMod {
        attrs,
        vis: syn::Visibility::Public(Pub::default()),
        unsafety,
        mod_token: Mod::default(),
        ident: ty,
        content: Some((Brace::default(), processed)),
        semi: None,
    }
}

fn process_type(
    t: ImplItemType,
    append_generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &mut ChangeSelfToContext,
) -> Item {
    let ImplItemType {
        attrs,
        ident,
        mut generics,
        mut ty,
        type_token,
        eq_token,
        semi_token,
        ..
    } = t;

    // change Self (to local or Context)
    folder.replaced = false;
    ty = folder.fold_type(ty);

    if folder.replaced {
        generics = process_generics(generics, true, append_generics, ty_generics, folder);
    } else {
        folder
            .local_idents
            .iter_mut()
            .find(|e| e.0 == ident)
            .expect("Not in local_idents! Should be impossible.")
            .1 = false;
    }

    Item::Type(ItemType {
        attrs,
        vis: Visibility::Public(Pub::default()),
        type_token,
        ident,
        generics,
        eq_token,
        ty: Box::new(ty),
        semi_token,
    })
}

fn process_fn(
    f: ImplItemFn,
    generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &mut ChangeSelfToContext,
) -> Item {
    let ImplItemFn {
        attrs,
        mut sig,
        mut block,
        ..
    } = f;

    sig.generics = process_generics(sig.generics, true, generics, ty_generics, folder);
    // change Self (to local or Context)
    sig.inputs = sig
        .inputs
        .into_iter()
        .map(|inp| folder.fold_fn_arg(inp))
        .collect();
    sig.output = match sig.output {
        ReturnType::Default => ReturnType::Default,
        ReturnType::Type(arr, t) => ReturnType::Type(arr, Box::new(folder.fold_type(*t))),
    };
    block.stmts = block
        .stmts
        .into_iter()
        .map(|stmt| folder.fold_stmt(stmt))
        .collect();

    Item::Fn(ItemFn {
        attrs,
        vis: Visibility::Public(Pub::default()),
        sig,
        block: Box::new(block),
    })
}

fn process_const(
    c: ImplItemConst,
    append_generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &mut ChangeSelfToContext,
) -> Item {
    let ImplItemConst {
        attrs,
        const_token,
        ident,
        mut generics,
        colon_token,
        ty,
        eq_token,
        mut expr,
        semi_token,
        ..
    } = c;

    generics = process_generics(generics, false, append_generics, ty_generics, folder);
    // change Self (to local or Context)
    expr = folder.fold_expr(expr);

    Item::Const(ItemConst {
        attrs,
        vis: Visibility::Public(Pub::default()),
        const_token,
        ident,
        generics,
        colon_token,
        ty: Box::new(ty),
        eq_token,
        expr: Box::new(expr),
        semi_token,
    })
}

fn process_generics(
    mut generics: Generics,
    insert_context: bool,
    append_generics: Generics,
    ty_generics: Punctuated<GenericArgument, Comma>,
    folder: &mut ChangeSelfToContext,
) -> Generics {
    if let Some(where_clause) = append_generics.where_clause {
        generics.where_clause = Some(WhereClause {
            where_token: Where::default(),
            predicates: generics
                .where_clause
                .map(|w| w.predicates)
                .unwrap_or(Punctuated::new())
                .into_iter()
                .chain(where_clause.predicates.into_iter())
                .collect(),
        });
    }
    // change Self (to local or Context)
    generics.params = insert_context
        .then_some(syn::GenericParam::Type(TypeParam::from(Ident::new(
            "Context",
            Span::mixed_site(),
        ))))
        .into_iter()
        .chain(ty_generics.into_iter().map(|arg| match arg {
            GenericArgument::Lifetime(l) => GenericParam::Lifetime(syn::LifetimeParam {
                attrs: vec![],
                lifetime: l,
                colon_token: None,
                bounds: Punctuated::new(),
            }),
            GenericArgument::Type(Type::Path(p)) => GenericParam::Type(syn::TypeParam {
                attrs: vec![],
                ident: p.path.segments[0].ident.clone(),
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }),
            _ => panic!("Only Type and Lifetime generics are supported on Impl"),
        }))
        .chain(append_generics.params)
        .chain(generics.params.into_iter().map(|param| match param {
            GenericParam::Type(mut t) => {
                t.default = t.default.map(|d| folder.fold_type(d));
                GenericParam::Type(t)
            }
            other => other,
        }))
        .collect();
    generics.where_clause = generics.where_clause.map(|mut w| {
        w.predicates = w
            .predicates
            .into_iter()
            .map(|pred| folder.fold_where_predicate(pred))
            .collect();
        w
    });
    generics
}
