mod index;

use std::collections::HashMap;

use index::IndexDef;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::*;

/// Add functions for initializing the KV for a database, and providing references to the KV to all
/// collections in the structure. Add additional methods for configuration safety checks, index
/// rebuilding, compacting, etc.
#[proc_macro_derive(AnonDB, attributes(anondb))]
pub fn anondb_collections(input: TokenStream) -> TokenStream {
    let crate_name = if std::env::var("CARGO_PKG_NAME").ok().as_deref() == Some("anondb") {
        quote! { crate } // Use crate:: when inside anondb
    } else {
        quote! { ::anondb } // Use ::anondb when external
    };

    let input = parse_macro_input!(input as DeriveInput);
    let fields = match parse_struct_and_fields(&input) {
        Ok(fields) => fields,
        Err(e) => return e.to_compile_error().into(),
    };

    let name = &input.ident;
    let kv_generic_name = match get_kv_generic(&input) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut field_primary_keys = HashMap::<Ident, IndexDef>::default();
    let mut field_indices = HashMap::<Ident, Vec<IndexDef>>::default();
    for field in fields {
        let (primary_key_maybe, indices) = match parse_attributes(field) {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };
        let field_ident = field.ident.clone().expect("expected field ident to exist");
        if let Some(primary_key) = primary_key_maybe {
            field_primary_keys.insert(field_ident.clone(), primary_key);
        }
        field_indices.insert(field_ident.clone(), indices);
    }

    // Iterate over fields and collect their names and types
    let assign_collection_primary_key = fields.iter().map(|f| {
        let field_name = &f.ident;
        let doc_generic = get_first_generic(&f.ty);
        if doc_generic.is_none() {
            return Error::new_spanned(f, "AnonDB unable to determine generic for field")
                .to_compile_error()
                .into();
        }
        let doc_generic = doc_generic.unwrap();
        if let Some(primary_key_parts) = field_primary_keys.get(field_name.as_ref().unwrap()) {
            let primary_key_fields = primary_key_parts.fields.iter().map(|v| v.name.clone()).collect::<Vec<_>>();
            let name = primary_key_fields.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("_");
            quote! {
                self.#field_name.set_primary_key_nonbuilder((#name.into(), |doc: &#doc_generic| -> Vec<u8> {
                    let mut bytes = Vec::default();
                    #(
                        bytes.extend(<_ as ::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.#primary_key_fields));
                    )*
                    bytes
                }))?;
            }
        } else {
            quote! {}
        }
    });

    let assign_collection_indices = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let doc_generic = get_first_generic(&f.ty);
        if doc_generic.is_none() {
            return Error::new_spanned(f, "AnonDB unable to determine generic for field")
                .to_compile_error()
                .into();
        }
        let doc_generic = doc_generic.unwrap();
        let index_assignments = field_indices
            .get(&field_name)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|index| {
                let fields = index
                    .fields
                    .iter()
                    .map(|i| i.name.clone())
                    .collect::<Vec<_>>();
                let field_strings = fields.iter().map(|v| v.to_string()).collect::<Vec<_>>();
                let name = vec![vec![field_name.to_string()], fields.iter().map(|v| v.to_string()).collect()]
                    .concat()
                    .join("_");
                let options = index.options.iter().map(|(k, v)| quote! { #k: #v }).collect::<Vec<_>>();
                quote! {
                    self.#field_name.add_index(
                        #crate_name::Index {
                            name: #name.into(),
                            field_names: vec![#(#field_strings.to_string(),)*],
                            serialize: |doc: &#doc_generic| -> Vec<u8> {
                                let mut bytes = Vec::default();
                                #(
                                    bytes.extend(<_ as ::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.#fields));
                                )*
                                bytes
                            },
                            options: #crate_name::IndexOptions {
                                #(#options,)*
                                ..Default::default()
                            },
                            _phantom: ::std::marker::PhantomData::default()
                        }
                    )?;
                }
            });
        quote! {
            #(#index_assignments)*
        }
    });

    // Iterate over fields and collect their names and types
    let assign_collection_kv = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            self.#field_name.set_kv(kv.clone())?;
            // println!("Generics: {}, Field: {}, Type: {}", stringify!(#impl_generics), stringify!(#field_name), stringify!(#field_type));
        }
    });

    let assign_collection_names = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            self.#field_name.set_name(stringify!(#field_name).into())?;
        }
    });

    let check_table_names = fields.iter().map(|f| {
        let field_name = &f.ident;
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
        quote! {
            self.#field_name.construct_indices()?;
        }
    });

    let check_primary_keys = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            if !self.#field_name.has_primary_key() {
                ::anyhow::bail!("Collection \"{}\" does not have a primary key defined!", self.#field_name.name());
            }
        }
    });

    let defaults = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            #field_name: #crate_name::Collection::default(),
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
                #(#assign_collection_indices)*
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

/// For all the collections in the db, extract primary_key attributes.
fn parse_attributes(field: &Field) -> Result<(Option<IndexDef>, Vec<IndexDef>)> {
    let mut primary_key_maybe: Option<IndexDef> = None;
    let mut indices = Vec::default();
    for attr in &field.attrs {
        if !attr.path().is_ident("anondb") {
            continue;
        }
        let index_def = attr.parse_args::<IndexDef>()?;
        match index_def.keyword.to_string().as_str() {
            "primary_key" => {
                if !index_def.options.is_empty() {
                    return Err(Error::new_spanned(
                        attr,
                        format!("AnonDB primary_key attribute does not support options"),
                    ));
                }
                primary_key_maybe = Some(index_def);
            }
            "index" => {
                indices.push(index_def);
            }
            key => {
                return Err(Error::new_spanned(
                    attr,
                    format!(
                        "AnonDB unrecognized key \"{key}\" in attribute for field \"{}\"",
                        stringify!(field)
                    ),
                ));
            }
        }
    }
    Ok((primary_key_maybe, indices))
}

/// Parse the derive invocation, the struct, and extract the fields.
fn parse_struct_and_fields(input: &DeriveInput) -> Result<&Punctuated<Field, Comma>> {
    // Only allow structs
    let data_struct = match &input.data {
        Data::Struct(s) => s,
        Data::Enum(_) => {
            return Err(Error::new_spanned(
                input,
                "AnonDB can only be derived for structs, not enums",
            ));
        }
        Data::Union(_) => {
            return Err(Error::new_spanned(
                input,
                "AnonDB can only be derived for structs, not unions",
            ));
        }
    };

    // Only allow named fields (bracket syntax)
    let fields = match &data_struct.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(_) => {
            return Err(Error::new_spanned(
                input,
                "AnonDB only works with structs that have named fields (with braces {}), not tuple structs",
            ));
        }
        Fields::Unit => {
            return Err(Error::new_spanned(
                input,
                "AnonDB only works with structs that have named fields (with braces {}), not unit structs",
            ));
        }
    };
    Ok(fields)
}

fn get_kv_generic(input: &DeriveInput) -> Result<&Ident> {
    // extract the generic for the database structure. This generic represents the KV implementation
    let generic = match input.generics.params.first() {
        Some(generic) => generic,
        None => {
            return Err(Error::new_spanned(
                input,
                "AnonDB only works with structs that have exactly 1 generic argument, which is trait bounded to anondb_kv::KV",
            ));
        }
    };

    let kv_generic_name = match generic {
        GenericParam::Type(type_param) => {
            let generic_name = &type_param.ident;
            if let Some(_bound) = type_param.bounds.first() {
                // TODO: check that this bound is anondb_kv::KV
            } else {
                return Err(Error::new_spanned(
                    type_param,
                    "AnonDB struct generic must have a first trait bound that is anondb_kv::KV",
                ));
            }
            generic_name
        }
        GenericParam::Const(v) => {
            return Err(Error::new_spanned(
                v,
                "AnonDB struct first generic must be a type, got a const",
            ));
        }
        GenericParam::Lifetime(v) => {
            return Err(Error::new_spanned(
                v,
                "AnonDB struct first generic must be a type, got a lifetime",
            ));
        }
    };
    Ok(kv_generic_name)
}

fn get_first_generic(ty: &Type) -> Option<&Type> {
    // Check if the type is a path (e.g., Vec<T>, Option<String>)
    if let Type::Path(type_path) = ty {
        // Get the last segment (e.g., "Vec" in "std::vec::Vec")
        if let Some(segment) = type_path.path.segments.last() {
            // Check if it has generic arguments
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                // Get the first generic argument
                if let Some(first_arg) = args.args.first() {
                    // Extract the type from the generic argument
                    if let GenericArgument::Type(ty) = first_arg {
                        return Some(ty);
                    }
                }
            }
        }
    }
    None
}
