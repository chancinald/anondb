use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::Result;
use syn::*;

use super::*;

/// Metadata collected for a single collection field on the `AnonDB` struct.
struct FieldMeta {
    /// The primary key index for this field.
    primary_key: IndexDef,
    /// All secondary indices defined for this field.
    indices: Vec<IndexDef>,
    /// The document type stored in this field's collection (the generic arg of `Collection<T>`).
    doc_generic: Type,
}

/// Add functions for initializing the KV for a database, and providing references to the KV to all
/// collections in the structure. Add additional methods for configuration safety checks, index
/// rebuilding, compacting, etc.
pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let crate_name = crate_name();

    // get all the collections in the db
    let fields = parse_struct_and_fields(&input, "AnonDB")?;

    // collect primary key, indices, and document type for each field
    let mut field_meta = HashMap::<Ident, FieldMeta>::default();
    for field in fields {
        let (primary_key, indices) = parse_attributes(field)?;
        let field_ident = field.ident.clone().expect("expected field ident to exist");
        let doc_generic = get_first_generic(&field.ty)
            .expect("expected field type to have a generic argument")
            .clone();
        field_meta.insert(
            field_ident,
            FieldMeta {
                primary_key,
                indices,
                doc_generic,
            },
        );
    }

    // custom indices may not be marked as primary; only #[anondb(primary_key = ...)] can do that
    for meta in field_meta.values() {
        for index in &meta.indices {
            for (option_name, _) in &index.options {
                if option_name == "primary" {
                    return Err(Error::new_spanned(
                        option_name,
                        "Custom indices may not be primary. Use the primary_key attribute instead.",
                    ));
                }
            }
        }
    }

    let defaults = fields.iter().map(|f| {
        let field_name = f.ident.clone().unwrap();
        let meta = field_meta
            .get(&field_name)
            .expect("expected field metadata to be known");
        let doc_generic = &meta.doc_generic;
        let primary_key_fields = meta
            .primary_key
            .fields
            .iter()
            .map(|v| v.name.clone())
            .collect::<Vec<_>>();
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
                        "AnonDB primary_key attribute does not support options",
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
    let primary_key = primary_key_maybe.ok_or_else(|| {
        Error::new_spanned(
            field,
            format!(
                "AnonDB collection \"{}\" does not have a primary key specified. You may do so with #[anondb(primary_key = field_name)]",
                field.ident.clone().unwrap()
            ),
        )
    })?;
    Ok((primary_key, indices))
}

/// Extract the generic for the database structure. This generic represents the KV implementation.
fn get_kv_generic(input: &DeriveInput) -> Result<&Ident> {
    let generic = input.generics.params.first().ok_or_else(|| {
        Error::new_spanned(
            input,
            "AnonDB only works with structs that have exactly 1 generic argument, which is trait bounded to anondb_kv::KV",
        )
    })?;

    match generic {
        GenericParam::Type(type_param) => {
            if type_param.bounds.first().is_none() {
                return Err(Error::new_spanned(
                    type_param,
                    "AnonDB struct generic must have a first trait bound that is anondb_kv::KV",
                ));
            }
            // TODO: check that this bound is anondb_kv::KV
            Ok(&type_param.ident)
        }
        GenericParam::Const(v) => Err(Error::new_spanned(
            v,
            "AnonDB struct first generic must be a type, got a const",
        )),
        GenericParam::Lifetime(v) => Err(Error::new_spanned(
            v,
            "AnonDB struct first generic must be a type, got a lifetime",
        )),
    }
}

/// Extract the first generic type argument from a path type, e.g. `T` from `Collection<T>`.
fn get_first_generic(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let GenericArgument::Type(ty) = args.args.first()? else {
        return None;
    };
    Some(ty)
}
