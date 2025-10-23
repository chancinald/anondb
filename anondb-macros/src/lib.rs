mod anondb;
mod document;
mod index;

use index::*;

use proc_macro::TokenStream;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::*;

#[proc_macro_derive(AnonDB, attributes(anondb))]
pub fn anondb_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match anondb::derive(input) {
        Ok(t) => t,
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Document)]
pub fn document_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match document::derive(input) {
        Ok(t) => t,
        Err(e) => e.to_compile_error().into(),
    }
}

fn crate_name() -> proc_macro2::TokenStream {
    if std::env::var("CARGO_PKG_NAME").ok().as_deref() == Some("anondb") {
        quote::quote! { crate }
    } else {
        quote::quote! { ::anondb }
    }
}

/// Parse the derive invocation, the struct, and extract the fields.
fn parse_struct_and_fields<'a>(
    input: &'a DeriveInput,
    macro_name: &str,
) -> Result<&'a Punctuated<Field, Comma>> {
    // Only allow structs
    let data_struct = match &input.data {
        Data::Struct(s) => s,
        Data::Enum(_) => {
            return Err(Error::new_spanned(
                input,
                format!("{macro_name} can only be derived for structs, not enums",),
            ));
        }
        Data::Union(_) => {
            return Err(Error::new_spanned(
                input,
                format!("{macro_name} can only be derived for structs, not unions",),
            ));
        }
    };

    // Only allow named fields (bracket syntax)
    let fields = match &data_struct.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(_) => {
            return Err(Error::new_spanned(
                input,
                format!(
                    "{macro_name} only works with structs that have named fields (with braces {{}}), not tuple structs",
                ),
            ));
        }
        Fields::Unit => {
            return Err(Error::new_spanned(
                input,
                format!(
                    "{macro_name} only works with structs that have named fields (with braces {{}}), not unit structs",
                ),
            ));
        }
    };
    Ok(fields)
}
