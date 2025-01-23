# Abstract Impl
This crate enables users to generate generic (abstract) implementations for traits, with bounds on the Self type.

The idea for this came primarily from [Context-Generic Programming](https://contextgeneric.dev/),
I wanted the core without all the bells and whistles (no providers, consumers and contexts).
I noticed that it primarily empowers the programmer to provide generic implementations for traits, that types can choose to implement.

The name abstract_impl comes from the possibility of having that in rust (abstract is a reserved keyword), you could have `abstract impl`.

## Use
```rust
use abstract_impl::abstract_impl;
// Create an implementation
#[abstract_impl]
impl DebugToString for ToString where Self: std::fmt::Debug {
                    // Require Bounds ~~~~~~~~~~~~~~~~~~~~~
    fn to_string(&self) -> String {
        //       ~~~~~     ~~~~~~
        // Use self like normal
        format!("{context:?}")
        //        ~~~~~~~
        // Sometimes (in macro invocations) you have to use context
    }
}
#[derive(Debug)]
struct Test(pub u8);
impl_DebugToString!(Test);
```
Features:
 - type errors at the right place
 - check that all items of the trait are implemented
 - use of self type with bounds
 - automatic implementation macro
### A more elaborate showcase
```rust
use abstract_impl::abstract_impl;
mod traits {
    pub trait FormatToString {
        type Error;
        fn format_to_string(&self) -> Result<String, Self::Error>;
    }

    pub trait Print {
        type Error;
        fn print(&self) -> Result<(), Self::Error>;
    }
}
use traits::*;

mod impls {
    use super::*;

    #[abstract_impl]
    impl FormatUsingDebug for FormatToString
    where
        Self: std::fmt::Debug,
    {
        type Error = ();
        fn format_to_string(&self) -> Result<String, Self::Error> {
            Ok(format!("{context:?}"))
        }
    }

    #[derive(Debug)]
    pub enum PrintUsingFormatErr<FormatErr> {
        Other,
        FormatErr(FormatErr),
    }
    #[abstract_impl]
    impl PrintUsingFormat for Print
    where
        Self: FormatToString,
    {
        type Error = PrintUsingFormatErr<<Self as FormatToString>::Error>;
        fn print(&self) -> Result<(), Self::Error> {
            let string = self.format_to_string().map_err(Self::Error::FormatErr)?;
            Ok(println!("{string}"))
        }
    }
    #[abstract_impl]
    impl PrintDummy for Print {
        type Error = ();
        fn print(&self) -> Result<(), ()> {
            Ok(println!("Hi"))
        }
    }
    #[abstract_impl(no_dummy)]
    impl PartialUsingOrd for std::cmp::PartialOrd
    where
        Self: Ord,
    {
        fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(rhs))
        }
    }
}
use impls::*;

#[derive(Debug)]
struct Person {
    pub first_name: String,
    pub last_name: String,
}

impl_FormatUsingDebug!(Person);
impl_PrintUsingFormat!(Person);

fn main() {
    Person {
        first_name: "Alice".to_string(),
        last_name: "B.".to_string(),
    }
    .print()
    .unwrap();
}
```
### No Macro
Sometimes the impl_Impl macros might not be desired.
In that case it may be disabled with the `no_macro` option.
```rust
use abstract_impl::abstract_impl;
#[abstract_impl(no_macro)]
impl DebugToString for ToString where Self: std::fmt::Debug {
    fn to_string(&self) -> String {
        format!("{context:?}")
    }
}
// impl_DebugToString(());
// No impl_Impl macro generated
// Manually use it instead
#[derive(Debug)]
struct Test;
impl ToString for Test {
    fn to_string(&self) -> String {
        DebugToString::to_string::<Self>(self)
    }
}
```
### No Dummy
`abstract_impl` automatically generates a dummy implementation for the trait, to check that all items are implemented.
This may result in problems for traits that have super traits.
In that case it may be disabled with the `no_dummy` option.
```rust
use abstract_impl::abstract_impl;
#[abstract_impl(no_dummy)]
impl PartialUsingOrd for std::cmp::PartialOrd
where
    Self: Ord,
{
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
```
If `no_dummy` wasn't used, you would get an error that Dummy doesn't implement Ord.
### Legacy Order
By default `impl Impl for Trait` is used.
Some people may prefer the previous order `impl Trait for Impl`.
```rust
use abstract_impl::abstract_impl;
#[abstract_impl(legacy_order)]
impl ToString for DebugToString where Self: std::fmt::Debug {
    fn to_string(&self) -> String {
        format!("{context:?}")
    }
}
```
## How it is made
(WIP)

## Future Plans
- [ ] const item in impl (currently has no generics)
- [ ] generics for trait and impl
- [ ] self in macros (expand inner first)
## Changelog
 - 0.1.0 Working version with basic documentation
 - 0.2.0 Reverse trait and name position (now impl Impl for Trait) and extended documentation
## License
This code is MIT licensed.
