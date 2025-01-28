#![doc = include_str!("../README.md")]
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::token;
use syn::{parse_macro_input, ItemImpl};
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
        attrs.iter().all(|attr| attr.to_string() != "no_dummy"),
        attrs.iter().all(|attr| attr.to_string() != "no_macro"),
        attrs.iter().any(|attr| attr.to_string() == "legacy_order"),
    ) {
        Ok(res) => res,
        Err(e) => return e.into_compile_error().into(),
    };
    res.to_token_stream().into()
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
