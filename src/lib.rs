#![allow(clippy::needless_doctest_main)]
#![doc = include_str!("../README.md")]
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, ItemImpl};
use syn::{token, Ident};
mod change_self;
mod dummy;
mod mac;
mod transform;

struct IdentList(Punctuated<syn::Ident, token::Comma>);
impl syn::parse::Parse for IdentList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(IdentList(
            input.parse_terminated(syn::Ident::parse, token::Comma)?,
        ))
    }
}

/// Define an abstract implementation for a trait, that types can use
///
/// ```
/// # use abstract_impl::abstract_impl;
/// # trait SomeTrait {
/// #   fn some() -> Self;
/// #   fn other(&self) -> String;
/// # }
/// #[abstract_impl]
/// impl Impl for SomeTrait where Self: Default + std::fmt::Debug {
///   fn some() -> Self {
///     Self::default()
///   }
///   fn other(&self) -> String {
///     // You have to use context here instead of self, because we don't change macro contents
///     format!("{context:?}")
///   }
/// }
/// impl_Impl!(());
/// # fn main() {
/// # assert_eq!("()", <()>::some().other());
/// # }
/// ```
#[proc_macro_attribute]
pub fn abstract_impl(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(item as ItemImpl);
    let IdentList(attrs) = parse_macro_input!(_attr);
    let res = match transform::transform(
        parsed,
        attrs.iter().all(|attr| *attr != "no_dummy"),
        attrs.iter().all(|attr| *attr != "no_macro"),
        attrs.iter().any(|attr| *attr == "legacy_order"),
    ) {
        Ok(res) => res,
        Err(e) => return e.into_compile_error().into(),
    };
    res.to_token_stream().into()
}

/// Generates a TyType trait (has type Ty) with a generic TyUsingType<T> impl given a type name Ty.
/// ```rust
/// # use abstract_impl::type_trait;
/// type_trait!(Ty);
/// struct Test;
/// impl_TyUsingType!(<u8> Test);
/// fn main() {
///   let x: <Test as TyType>::Ty = 5u8;
/// # assert_eq!(x, 5);
/// }
/// ```
#[proc_macro]
pub fn type_trait(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ty = parse_macro_input!(item as Ident);
    let trait_name = Ident::new(&format!("{ty}Type"), ty.span());
    let impl_name = Ident::new(&format!("{ty}UsingType"), ty.span());
    quote! {
        pub trait #trait_name {
            type #ty;
        }
        #[abstract_impl::abstract_impl(no_dummy)]
        impl #impl_name<T> for #trait_name {
            type #ty = T;
        }
    }
    .into()
}

// DOCTESTS(hidden):
/// ```rust
/// use abstract_impl::abstract_impl;
/// trait TimeType {
///   type Time;
/// }
/// #[abstract_impl(no_dummy)]
/// impl TimeUsingType<T> for TimeType {
///   type Time = T;
/// }
/// impl_TimeUsingType!(<usize> ());
/// fn main() {}
/// ```
#[allow(dead_code)]
struct Tests;
