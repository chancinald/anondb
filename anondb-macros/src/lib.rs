mod anondb;
mod index;

use index::*;

use proc_macro::TokenStream;
use syn::DeriveInput;
use syn::parse_macro_input;

#[proc_macro_derive(AnonDB, attributes(anondb))]
pub fn anondb_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match anondb::anondb(input) {
        Ok(t) => t,
        Err(e) => e.to_compile_error().into(),
    }
}
