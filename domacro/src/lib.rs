use proc_macro::TokenStream;
use quote::quote;
use syn::ItemFn;
use syn::Path;
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn domacro(attr: TokenStream, item: TokenStream) -> TokenStream {
    let macro_name = parse_macro_input!(attr as Path);
    let function = parse_macro_input!(item as ItemFn);

    let fn_name = &function.sig.ident;

    let expanded = quote! {
        #function

        #macro_name!(#fn_name);
    };

    TokenStream::from(expanded)
}
