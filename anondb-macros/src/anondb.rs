use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::Result;
use syn::*;

use super::*;

/// Add functions for initializing the KV for a database, and providing references to the KV to all
/// collections in the structure. Add additional methods for configuration safety checks, index
/// rebuilding, compacting, etc.
pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let crate_name = crate_name();

    // get all the collections in the db
    let fields = parse_struct_and_fields(&input, "AnonDB")?;

    // the primary key that is defined for each field
    let mut field_primary_keys = HashMap::<Ident, IndexDef>::default();
    // all indices defined for each field
    let mut field_indices = HashMap::<Ident, Vec<IndexDef>>::default();
    // the type of the document for each field
    let mut field_doc_generic = HashMap::<Ident, Type>::default();
    for field in fields {
        let (primary_key, indices) = parse_attributes(field)?;
        let field_ident = field.ident.clone().expect("expected field ident to exist");
        field_primary_keys.insert(field_ident.clone(), primary_key);
        field_indices.insert(field_ident.clone(), indices);
        let doc_generic = get_first_generic(&field.ty).unwrap();
        field_doc_generic.insert(field_ident.clone(), doc_generic.clone());
    }

    for (_collection_name, indices) in &field_indices {
        for index in indices {
            for (option_name, _) in &index.options {
                if option_name.to_string() == "primary" {
                    return Err(Error::new_spanned(
                        option_name,
                        format!("Custom indices may not be primary. Use the primary_key attribute instead."),
                    ));
                }
            }
        }
    }

    let assign_collection_vars = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let doc_generic = field_doc_generic.get(&field_name).expect("expected field document type to be known");
        let primary_key_parts = field_primary_keys.get(&field_name).unwrap();
        let primary_key_fields = primary_key_parts.fields.iter().map(|v| v.name.clone()).collect::<Vec<_>>();
        let mut all_indexed_fields = HashMap::<Ident, ()>::default();
        for index in field_indices.get(&field_name).cloned().unwrap_or_default() {
            for field in index.fields {
                all_indexed_fields.insert(field.name, ());
            }
        }

        let field_extractors = all_indexed_fields.iter().map(|(k, _)| {
            quote! {
                if let Some(v) = query.#k.as_ref() {
                    out.insert(stringify!(#k).to_string(), v.into());
                }
            }
        });
        let extract_index_fields = quote! {
            {
                fn extractor(query: & <#doc_generic as #crate_name::Queryable> ::DocumentQuery) -> std::collections::HashMap<String, #crate_name::Param> {
                    let mut out = std::collections::HashMap::default();
                    #(#field_extractors)*
                    out
                }
                self.#field_name.set_field_extractor(extractor);
            }
        };

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

                let options = index.options.iter().map(|(k, v)| quote! { #k: #v }).collect::<Vec<_>>();
                quote! {
                    self.#field_name.add_index(
                        #crate_name::Index {
                            collection_name: stringify!(#field_name).into(),
                            field_names: vec![
                                #(
                                    (
                                        stringify!(#fields).to_string(),
                                        <<#doc_generic as #crate_name::Queryable>::DocumentPhantom>::#fields ().stats()
                                    ),
                                )*
                            ],
                            serialize: |doc: &#doc_generic| -> Vec<u8> {
                                let mut key = #crate_name::anondb_kv::LexicographicKey::default();
                                #({
                                    let bytes = <_ as #crate_name::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.#fields);
                                    key.append_key_slice(bytes.as_slice());
                                })*
                                key.take()
                            },
                            options: #crate_name::IndexOptions {
                                #(#options,)*
                                ..Default::default()
                            }
                        }
                    )?;
                }
            });
        quote! {
            {
                // needed to access stats functions
                use #crate_name::SerializeLexicographic;
                // assign the kv
                self.#field_name.set_kv(kv.clone())?;
                // assign the collection name as a string
                self.#field_name.set_name(stringify!(#field_name).into())?;
                // assign the primary key
                self.#field_name.set_primary_key((
                        vec![#(
                            (
                                stringify!(#primary_key_fields).to_string(), 
                                <<#doc_generic as #crate_name::Queryable>::DocumentPhantom>::#primary_key_fields ().stats()
                             )
                            ),*], |doc: &#doc_generic| -> Vec<u8> {
                    let mut key = #crate_name::anondb_kv::LexicographicKey::default();
                    #(
                        let bytes = <_ as #crate_name::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.#primary_key_fields);
                        key.append_key_slice(bytes.as_slice());
                    )*
                    key.take()
                }))?;
                #extract_index_fields
                // assign all indices
                #(#index_assignments)*
            }
        }
    });

    let collection_checks = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            for table_name in self.#field_name.table_names() {
                if let Some(collection) = all_table_names.get(&table_name) {
                    #crate_name::anyhow::bail!("AnonDB: invalid configuration. Table name \"{}\" is used by two different collections: \"{}\" and \"{}\"", table_name, collection, stringify!(#field_name));
                }
                all_table_names.insert(table_name.into(), stringify!(#field_name).into());
            }
            self.#field_name.construct_indices()?;
            if !self.#field_name.has_primary_key() {
                #crate_name::anyhow::bail!("Collection \"{}\" does not have a primary key defined!", self.#field_name.name());
            }
        }
    });

    let defaults = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            #field_name: #crate_name::Collection::default(),
        }
    });

    // the name of the database struct
    let name = &input.ident;

    // the concrete KV implementation for the database
    let kv_generic_name = get_kv_generic(&input)?;

    // types on the struct
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

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
            pub fn in_memory(bytes_maybe: Option<&[u8]>) -> #crate_name::anyhow::Result<::std::sync::Arc<Self>> {
                let mut s = Self::default();
                let kv = ::std::sync::Arc::new(#kv_generic_name::in_memory(bytes_maybe)?);
                s.setup(kv)?;
                Ok(::std::sync::Arc::new(s))
            }

            /// Initialize the database backed by a kv that exists on disk.
            pub fn at_path(path: &::std::path::Path) -> #crate_name::anyhow::Result<::std::sync::Arc<Self>> {
                let mut s = Self::default();
                let kv = ::std::sync::Arc::new(#kv_generic_name::at_path(path)?);
                s.setup(kv)?;
                Ok(::std::sync::Arc::new(s))
            }

            /// Assign collection variables based on struct values.
            fn setup(&mut self, kv: ::std::sync::Arc<#kv_generic_name>) -> #crate_name::anyhow::Result<()> {
                // assign values to the collection such as kv, name, indices
                #(#assign_collection_vars)*

                let mut all_table_names = ::std::collections::HashMap::<String, String>::default();
                // Check the consistency of the database configuration. Check for conflicting
                // collection/index names. In the future read a configuration from the kv to
                // automatically detect schema changes and check for inconsistencies.
                #(#collection_checks)*
                Ok(())
            }
        }
    };

    Ok(TokenStream::from(expanded))
}

/// For all the collections in the db, extract primary_key attributes.
fn parse_attributes(field: &Field) -> Result<(IndexDef, Vec<IndexDef>)> {
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
    if primary_key_maybe.is_none() {
        return Err(Error::new_spanned(
            field,
            format!(
                "AnonDB collection \"{}\" does not have a primary key specified. You may do so with #[anondb(primary_key = field_name)]",
                field.ident.clone().unwrap().to_string()
            ),
        ));
    }
    Ok((primary_key_maybe.unwrap(), indices))
}

/// extract the generic for the database structure. This generic represents the KV implementation
fn get_kv_generic(input: &DeriveInput) -> Result<&Ident> {
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
