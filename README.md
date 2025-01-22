# Abstract Impl
This crate enables users to generate generic (abstract) implementations for traits, with bounds on the Self type.

The idea for this came primarily from [Context-Generic Programming](https://contextgeneric.dev/),
I wanted the core without all the bells and whistles (no providers, consumers and contexts).
I noticed that it primarily empowers the programmer to provide generic implementations for traits, that types can choose to implement.

The name abstract_impl comes from the possibility of having that in rust (abstract is a reserved keyword), you could have `abstract impl`.

## Use
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
    impl FormatToString for FormatUsingDebug
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
    impl Print for PrintUsingFormat
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
    impl Print for PrintDummy {
        type Error = ();
        fn print(&self) -> Result<(), ()> {
            Ok(println!("Hi"))
        }
    }
    #[abstract_impl(no_dummy)]
    impl std::cmp::PartialOrd for PartialUsingOrd
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

### No Dummy

## How it is made

## License
This code is MIT licensed.
