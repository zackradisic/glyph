use proc_macro::{Literal, TokenStream, TokenTree};
use quote::{format_ident, quote};

/// Converts highlight names
#[proc_macro]
pub fn make_highlights(stream: TokenStream) -> TokenStream {
    let highlights_raw: Vec<Literal> = stream
        .into_iter()
        .filter_map(|tt| match tt {
            TokenTree::Literal(lit) => Some(lit),
            _ => None,
        })
        .collect();

    let count = highlights_raw.len();

    let mut array_tree = quote! {};
    let mut enum_tree = quote! {};
    let mut convert_tree = quote! {};
    let mut reverse_convert_tree = quote! {};

    let mut stream = TokenStream::new();
    for (i, lit) in highlights_raw.into_iter().enumerate() {
        let raw = lit.to_string().replace("\"", "");
        let enum_name = format_ident!(
            "{}",
            raw.split('.')
                .map(|s| {
                    s.chars()
                        .next()
                        .iter()
                        .map(|c| c.to_ascii_uppercase())
                        .chain(s.chars().skip(1))
                        .collect::<String>()
                })
                .collect::<String>()
        );
        let i = i as u8;

        array_tree = quote! {
            #array_tree
            #raw,
        };
        enum_tree = quote! {
            #enum_tree
            #enum_name,
        };
        reverse_convert_tree = quote! {
            #reverse_convert_tree
            #i => Some(Highlight::#enum_name),
        };
        convert_tree = quote! {
            #convert_tree
            Highlight::#enum_name => #i,
        };
    }

    let array_tokens = quote! {
        pub const HIGHLIGHTS: &[&str; #count] = &[
            #array_tree
        ];
    };
    let enum_tokens = quote! {
        #[derive(Debug)]
        pub enum Highlight {
            #enum_tree
        }

        impl Highlight {
            #[inline]
            pub fn from_u8(val: u8) -> Option<Self> {
                match val {
                    #reverse_convert_tree
                    _ => None
                }
            }

            #[inline]
            pub fn to_u8(&self) -> u8 {
                match self {
                    #convert_tree
                }
            }
        }
    };

    stream.extend(TokenStream::from(enum_tokens));
    stream.extend(TokenStream::from(array_tokens));
    stream
}

#[proc_macro]
pub fn make_request(stream: TokenStream) -> TokenStream {
    let mut req_tt = quote! {};
    let mut count: u8 = 0;

    for tt in stream.into_iter() {
        if let TokenTree::Ident(ident) = tt {
            count += 1;
            let ident = format_ident!("{}", ident.to_string());
            req_tt = quote! {
                #req_tt
                #ident,
            }
        }
    }

    let req_tt = req_tt;
    TokenStream::from(quote! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum Request {
            #req_tt
        }

        impl Request {
            fn from_u8(val: u8) -> Result<Self, anyhow::Error> {
                if val >= #count {
                    Err(anyhow::anyhow!("Invalid value: {}", val))
                } else {
                    unsafe { Ok(std::mem::transmute::<u8, Request>(val)) }
                }
            }
        }
    })
}

#[proc_macro]
pub fn make_notification(stream: TokenStream) -> TokenStream {
    let mut req_tt = quote! {};
    let mut count: u8 = 0;

    for tt in stream.into_iter() {
        if let TokenTree::Ident(ident) = tt {
            count += 1;
            let ident = format_ident!("{}", ident.to_string());
            req_tt = quote! {
                #req_tt
                #ident,
            }
        }
    }

    let req_tt = req_tt;
    TokenStream::from(quote! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum Notification {
            #req_tt
        }

        impl Notification {
            fn from_u8(val: u8) -> Result<Self, anyhow::Error> {
                if val >= #count {
                    Err(anyhow::anyhow!("Invalid value: {}", val))
                } else {
                    unsafe { Ok(std::mem::transmute::<u8, Notification>(val)) }
                }
            }
        }
    })
}
