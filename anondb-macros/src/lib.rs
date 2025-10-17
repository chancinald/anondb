use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::Data;
use syn::DeriveInput;
use syn::Expr;
use syn::ExprPath;
use syn::Fields;
use syn::Ident;
use syn::parse_macro_input;

/// Add functions for initializing the KV for a database, and providing references to the KV to all
/// collections in the structure. Add additional methods for configuration safety checks, index
/// rebuilding, compacting, etc.
#[proc_macro_derive(AnonDB, attributes(anondb))]
pub fn anondb_collections(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Only allow structs
    let data_struct = match &input.data {
        Data::Struct(s) => s,
        Data::Enum(_) => {
            return syn::Error::new_spanned(
                input,
                "AnonDB can only be derived for structs, not enums",
            )
            .to_compile_error()
            .into();
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(
                input,
                "AnonDB can only be derived for structs, not unions",
            )
            .to_compile_error()
            .into();
        }
    };

    // Only allow named fields (bracket syntax)
    let fields = match &data_struct.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(_) => {
            return syn::Error::new_spanned(
                input,
                "AnonDB only works with structs that have named fields (with braces {}), not tuple structs"
            )
            .to_compile_error()
            .into();
        }
        Fields::Unit => {
            return syn::Error::new_spanned(
                input,
                "AnonDB only works with structs that have named fields (with braces {}), not unit structs"
            )
            .to_compile_error()
            .into();
        }
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let generic = if let Some(generic) = input.generics.params.first() {
        generic
    } else {
        return syn::Error::new_spanned(
            input,
            "AnonDB only works with structs that have exactly 1 generic argument, which is trait bounded to anondb_kv::KV"
        )
        .to_compile_error()
        .into();
    };

    let kv_generic_name = match generic {
        syn::GenericParam::Type(type_param) => {
            let generic_name = &type_param.ident;
            if let Some(_bound) = type_param.bounds.first() {
                // TODO: check that this bound is anondb_kv::KV
            } else {
                return syn::Error::new_spanned(
                    type_param,
                    "AnonDB struct generic must have a first trait bound that is anondb_kv::KV",
                )
                .to_compile_error()
                .into();
            }
            generic_name
        }
        syn::GenericParam::Const(v) => {
            return syn::Error::new_spanned(
                v,
                "AnonDB struct first generic must be a type, got a const",
            )
            .to_compile_error()
            .into();
        }
        syn::GenericParam::Lifetime(v) => {
            return syn::Error::new_spanned(
                v,
                "AnonDB struct first generic must be a type, got a lifetime",
            )
            .to_compile_error()
            .into();
        }
    };
    let mut field_primary_keys = HashMap::<Ident, ExprPath>::default();
    for field in fields {
        for attr in &field.attrs {
            if !attr.path().is_ident("anondb") {
                continue;
            }
            let meta = attr.meta.clone();
            match meta {
                syn::Meta::List(list) => {
                    let nested: syn::Result<syn::MetaNameValue> = syn::parse2(list.tokens.clone());
                    if let Ok(name_value) = nested {
                        let key = name_value.path.get_ident().unwrap().to_string();
                        if key == "primary_key" {
                            if let Expr::Path(expr_path) = &name_value.value {
                                field_primary_keys.insert(
                                    field
                                        .ident
                                        .clone()
                                        .expect("AnonDB proc-macro field did not have an ident"),
                                    expr_path.clone(),
                                );
                            } else {
                                return syn::Error::new_spanned(
                                    name_value,
                                    format!(
                                        "AnonDB value for primary_key must be a path (e.g. sequence of identifiers like value.inner.other): in field \"{}\"",
                                        stringify!(field)
                                    ),
                                )
                                .to_compile_error()
                                .into();
                            }
                        } else {
                            return syn::Error::new_spanned(
                                list,
                                format!(
                                    "AnonDB unrecognized key \"{key}\" in attribute for field \"{}\"",
                                    stringify!(field)
                                ),
                            )
                            .to_compile_error()
                            .into();
                        }
                    } else {
                        return syn::Error::new_spanned(
                            list,
                            format!(
                                "AnonDB could not parse struct attribute for field \"{}\"",
                                stringify!(field)
                            ),
                        )
                        .to_compile_error()
                        .into();
                    }
                }
                syn::Meta::Path(_) => {
                    return syn::Error::new_spanned(
                        meta,
                        "AnonDB struct attribute should be a List, got Path",
                    )
                    .to_compile_error()
                    .into();
                }
                syn::Meta::NameValue(_) => {
                    return syn::Error::new_spanned(
                        meta,
                        "AnonDB struct attribute should be a List, got NameValue",
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }
    }

    // Iterate over fields and collect their names and types
    let assign_collection_primary_key = fields.iter().map(|f| {
        let field_name = &f.ident;
        let doc_generic = get_first_generic(&f.ty);
        if doc_generic.is_none() {
            return syn::Error::new_spanned(f, "AnonDB unable to determine generic for field")
                .to_compile_error()
                .into();
        }
        let doc_generic = doc_generic.unwrap();
        if let Some(primary_key_parts) = field_primary_keys.get(field_name.as_ref().unwrap()) {
            quote! {
                self.#field_name.set_primary_key_nonbuilder((stringify!(#primary_key_parts).into(), |doc: &#doc_generic| -> Vec<u8> {
                    <_ as ::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.#primary_key_parts)
                }))?;
            }
        } else {
            quote! {}
        }
    });

    // Iterate over fields and collect their names and types
    let assign_collection_kv = fields.iter().map(|f| {
        let field_name = &f.ident;
        let _field_type = &f.ty;
        quote! {
            self.#field_name.set_kv(kv.clone())?;
            // println!("Generics: {}, Field: {}, Type: {}", stringify!(#impl_generics), stringify!(#field_name), stringify!(#field_type));
        }
    });

    let assign_collection_names = fields.iter().map(|f| {
        let field_name = &f.ident;
        let _field_type = &f.ty;
        quote! {
            self.#field_name.set_name(stringify!(#field_name).into())?;
        }
    });

    let check_table_names = fields.iter().map(|f| {
        let field_name = &f.ident;
        let _field_type = &f.ty;
        quote! {
            for table_name in self.#field_name.table_names() {
                if let Some(collection) = all_table_names.get(&table_name) {
                    ::anyhow::bail!("AnonDB: invalid configuration. Table name \"{}\" is used by two different collections: \"{}\" and \"{}\"", table_name, collection, stringify!(#field_name));
                }
                all_table_names.insert(table_name.into(), stringify!(#field_name).into());
            }
        }
    });

    let construct_indices = fields.iter().map(|f| {
        let field_name = &f.ident;
        let _field_type = &f.ty;
        quote! {
            self.#field_name.construct_indices()?;
        }
    });

    let check_primary_keys = fields.iter().map(|f| {
        let field_name = &f.ident;
        let _field_type = &f.ty;
        quote! {
            if !self.#field_name.has_primary_key() {
                ::anyhow::bail!("Collection \"{}\" does not have a primary key defined!", self.#field_name.name());
            }
        }
    });

    let defaults = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            #field_name: Collection::default(),
        }
    });

    let expanded = quote! {
        impl #impl_generics Default for #name #ty_generics #where_clause {
            fn default() -> Self {
                Self {
                    #(#defaults)*
                }
            }
        }

        impl #impl_generics #name #ty_generics #where_clause {
            /// Initialize the database backed by a kv that exists in memory.
            pub fn in_memory(bytes_maybe: Option<&[u8]>) -> ::anyhow::Result<::std::sync::Arc<Self>> {
                let mut s = Self::default();
                let kv = ::std::sync::Arc::new(#kv_generic_name::in_memory(bytes_maybe)?);
                s.setup(kv)?;
                s.check_consistency()?;
                Ok(::std::sync::Arc::new(s))
            }

            /// Initialize the database backed by a kv that exists on disk.
            pub fn at_path(path: &::std::path::Path) -> ::anyhow::Result<::std::sync::Arc<Self>> {
                let mut s = Self::default();
                let kv = ::std::sync::Arc::new(#kv_generic_name::at_path(path)?);
                s.setup(kv)?;
                s.check_consistency()?;
                Ok(::std::sync::Arc::new(s))
            }

            /// Assign collection variables based on struct values.
            fn setup(&mut self, kv: ::std::sync::Arc<#kv_generic_name>) -> ::anyhow::Result<()> {
                #(#assign_collection_names)*
                #(#assign_collection_kv)*
                #(#assign_collection_primary_key)*
                #(#construct_indices)*
                #(#check_primary_keys)*
                Ok(())
            }

            /// Check the consistency of the database configuration. Check for conflicting
            /// collection/index names. In the future read a configuration from the kv to
            /// automatically detect schema changes and check for inconsistencies.
            fn check_consistency(&self) -> ::anyhow::Result<()> {
                let mut all_table_names = ::std::collections::HashMap::<String, String>::new();
                #(#check_table_names)*

                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}

fn get_first_generic(ty: &syn::Type) -> Option<&syn::Type> {
    // Check if the type is a path (e.g., Vec<T>, Option<String>)
    if let syn::Type::Path(type_path) = ty {
        // Get the last segment (e.g., "Vec" in "std::vec::Vec")
        if let Some(segment) = type_path.path.segments.last() {
            // Check if it has generic arguments
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                // Get the first generic argument
                if let Some(first_arg) = args.args.first() {
                    // Extract the type from the generic argument
                    if let syn::GenericArgument::Type(ty) = first_arg {
                        return Some(ty);
                    }
                }
            }
        }
    }
    None
}
