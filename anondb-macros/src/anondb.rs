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

    let defaults = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let doc_generic = field_doc_generic.get(&field_name).expect("expected field document type to be known");
        let primary_key_parts = field_primary_keys.get(&field_name).unwrap();
        let primary_key_fields = primary_key_parts.fields.iter().map(|v| v.name.clone()).collect::<Vec<_>>();
        quote! {
            #field_name: #crate_name::Collection::new(
                kv.clone(),
                stringify!(#field_name).into(), 
                #crate_name::Index::new_primary(
                    stringify!(#field_name).into(),
                    vec![#(
                        (
                            stringify!(#primary_key_fields).to_string(), 
                            <<#doc_generic as #crate_name::Queryable>::DocumentPhantom>::#primary_key_fields ().stats()
                         )
                        ),*
                    ],
                    |doc: &#doc_generic| -> Vec<u8> {
                        let mut key = #crate_name::anondb_kv::LexicographicKey::default();
                        #(
                            let bytes = <_ as #crate_name::anondb_kv::SerializeLexicographic>::serialize_lex(&doc.#primary_key_fields);
                            key.append_key_slice(bytes.as_slice());
                        )*
                        key.take()
                    })
                ),
        }
    });

    // the name of the database struct
    let name = &input.ident;

    // the concrete KV implementation for the database
    let kv_generic_name = get_kv_generic(&input)?;

    // types on the struct
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// Initialize the database backed by a kv that exists in memory.
            pub async fn new(config_maybe: Option<#kv_generic_name::Config>) -> Result<Self> {
                let kv = #kv_generic_name::new(config_maybe).await?;
                Ok(Self {
                    #(#defaults)*
                })
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
