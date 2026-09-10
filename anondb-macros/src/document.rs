use proc_macro::TokenStream;
use quote::quote;
use syn::Result;
use syn::*;

use super::*;

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let crate_name = crate_name();

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let fields = parse_struct_and_fields(&input, "Document")?;
    let field_count = fields.len();

    let field_constraints = fields.iter().map(|f| {
        let field_type = f.ty.clone();
        quote! {
            const _: fn() = || {
                fn assert_valid_type<T: #crate_name::ValidDBType>() {}
                assert_valid_type::<#field_type>();
            };
        }
    });

    let phantom_associated_functions = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let field_type = f.ty.clone();
        quote! {
            pub fn #field_name () -> ::std::marker::PhantomData<#field_type> {
                ::std::marker::PhantomData::<#field_type>
            }
        }
    });

    let set_fields = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        // let field_type = f.ty.clone();
        quote! {
            if let Some(param) = self.#field_name.as_ref() {
                out.push((stringify!(#field_name), param.into()));
            }
        }
    });

    let query_fields = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let field_type = f.ty.clone();
        quote! {
            pub #field_name: Option<#crate_name::ParamTyped<#field_type>>
        }
    });

    let match_entries = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let _field_type = f.ty.clone();
        quote! {
            if let Some(param) = query.#field_name.as_ref() {
                if !param.test(&self.#field_name) {
                    return false;
                }
            }
        }
    });
    let query_methods = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let field_type = f.ty.clone();
        quote! {
            pub fn #field_name (mut self, p: impl Into<#crate_name::ParamTyped<#field_type>>) -> Self {
                self.#field_name = Some(p.into());
                self
            }
        }
    });
    let query_struct_name = quote::format_ident!("{}_Query", name);
    let phantom_struct_name = quote::format_ident!("{}_Phantom", name);

    let expanded = quote! {

        #[derive(Default, Clone, Debug)]
        #[allow(non_camel_case_types)]
        pub struct #impl_generics #query_struct_name #ty_generics #where_clause {
            #(#query_fields),*
        }

        impl #impl_generics #query_struct_name #ty_generics #where_clause {
            #(#query_methods)*
        }

        impl #crate_name::Query for #impl_generics #query_struct_name #ty_generics #where_clause {
            /// Get a vector containing the fields that have been set.
            fn fields(&self) -> Vec<(&'static str, Param)> {
                let mut out = Vec::with_capacity(#field_count);
                #(#set_fields)*
                out
            }
        }

        #(#field_constraints)*

        #[derive(Debug)]
        #[allow(non_camel_case_types)]
        pub struct #phantom_struct_name;

        impl #phantom_struct_name {
            #(#phantom_associated_functions)*
        }

        impl #impl_generics #crate_name::Queryable for #name #ty_generics #where_clause {
            type DocumentQuery = #query_struct_name;
            type DocumentPhantom = #phantom_struct_name;

            fn matches(&self, query: &Self::DocumentQuery) -> bool {
                #(#match_entries)*
                true
            }
        }
    };

    Ok(TokenStream::from(expanded))
}
