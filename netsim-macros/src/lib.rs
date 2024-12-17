use {
    std::{
        net::{Ipv4Addr, Ipv6Addr},
        str::FromStr,
    },
    proc_macro::TokenStream,
    syn::spanned::Spanned,
    quote::quote_spanned,
};

/// Creates a `Ipv4Network` given an address range in CIDR notation.
///
/// # Example
///
/// ```rust
/// assert_eq!(Ipv4Network::new(ipv4!("192.168.0.0"), 16), ipv4_network!("192.168.0.0/16"));
/// ```
#[proc_macro]
pub fn ipv4_network(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::LitStr);
    let span = input.span();
    let s = input.value();
    let output = match parse_ipv4_network(&s) {
        Ok((addr, subnet_mask_bits)) => {
            let [b0, b1, b2, b3] = addr.octets();
            quote_spanned!(span=> {
                ::netsim::Ipv4Network::new(::std::net::Ipv4Addr::new(#b0, #b1, #b2, #b3), #subnet_mask_bits)
            })
        },
        Err(err) => {
            quote_spanned!(span=> {
                compile_error!(#err)
            })
        },
    };
    output.into()
}

fn parse_ipv4_network(s: &str) -> Result<(Ipv4Addr, u8), String> {
    let (addr, subnet_mask_bits) = match s.split_once('/') {
        None => return Err(String::from("missing '/' character")),
        Some((addr, subnet_mask_bits)) => (addr, subnet_mask_bits),
    };
    let addr = match Ipv4Addr::from_str(addr) {
        Err(err) => return Err(err.to_string()),
        Ok(addr) => addr,
    };
    let subnet_mask_bits = match u8::from_str(subnet_mask_bits) {
        Err(err) => return Err(err.to_string()),
        Ok(subnet_mask_bits) => subnet_mask_bits,
    };
    if subnet_mask_bits > 32 {
        return Err(String::from("subnet mask bits cannot be greater than 32"));
    }
    Ok((addr, subnet_mask_bits))
}

/// Creates a `Ipv6Network` given an address range in CIDR notation.
///
/// # Example
///
/// ```rust
/// assert_eq!(Ipv6Network::new(ipv6!("ff00::"), 8), ipv6_network!("ff00::/8"));
/// ```
#[proc_macro]
pub fn ipv6_network(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::LitStr);
    let span = input.span();
    let s = input.value();
    let output = match parse_ipv6_network(&s) {
        Ok((addr, subnet_mask_bits)) => {
            let [b0, b1, b2, b3, b4, b5, b6, b7] = addr.segments();
            quote_spanned!(span=> {
                ::netsim::Ipv6Network::new(
                    ::std::net::Ipv6Addr::new(#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7),
                    #subnet_mask_bits,
                )
            })
        },
        Err(err) => {
            quote_spanned!(span=> {
                compile_error!(#err)
            })
        },
    };
    output.into()
}

fn parse_ipv6_network(s: &str) -> Result<(Ipv6Addr, u8), String> {
    let (addr, subnet_mask_bits) = match s.split_once('/') {
        None => return Err(String::from("missing '/' character")),
        Some((addr, subnet_mask_bits)) => (addr, subnet_mask_bits),
    };
    let addr = match Ipv6Addr::from_str(addr) {
        Err(err) => return Err(err.to_string()),
        Ok(addr) => addr,
    };
    let subnet_mask_bits = match u8::from_str(subnet_mask_bits) {
        Err(err) => return Err(err.to_string()),
        Ok(subnet_mask_bits) => subnet_mask_bits,
    };
    if subnet_mask_bits > 128 {
        return Err(String::from("subnet mask bits cannot be greater than 128"));
    }
    Ok((addr, subnet_mask_bits))
}

/// Makes a function run in an isolated network environment.
#[proc_macro_attribute]
pub fn isolate(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let item_fn = syn::parse_macro_input!(input as syn::ItemFn);
    let span = item_fn.span();
    let syn::ItemFn { attrs, vis, sig, block } = item_fn;
    let is_async = sig.asyncness.is_some();
    let output = if is_async {
        quote_spanned! {span=>
            #(#attrs)*
            #vis #sig {
                let machine = netsim::Machine::new().expect("error creating machine");
                let join_handle = machine.spawn(async move #block);
                join_handle.await.unwrap().unwrap()
            }
        }
    } else {
        quote_spanned! {span=>
            #(#attrs)*
            #vis #sig {
                let machine = netsim::Machine::new().expect("error creating machine");
                let join_handle = machine.spawn(async move {
                    ::netsim::tokio::task::spawn_blocking(move || #block).await.unwrap()
                });
                join_handle.join_blocking().unwrap().unwrap()
            }
        }
    };
    output.into()
}

