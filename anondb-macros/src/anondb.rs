use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::Result;
use syn::spanned::Spanned;

use super::*;

/// Metadata collected for a single collection field on the `AnonDB` struct.
struct FieldMeta {
    /// The primary key index for this field.
    primary_key_index: IndexDef,
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
    for field in fields.into_iter() {
        let doc_generic = get_first_generic(&field.ty)
            .expect("expected field type to have a generic argument")
            .clone();
        assert_no_generics(&doc_generic)?;
        let field_ident = field.ident.clone().expect("expected field ident to exist");
        let (primary_key_index, indices) = parse_attributes(field)?;
        field_meta.insert(
            field_ident,
            FieldMeta {
                primary_key_index,
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

    let collections = field_meta.iter().map(|(collection_name, meta)| {
        let doc_generic = &meta.doc_generic;
        let primary_index =
            meta.primary_key_index
                .generate_index_constructor(collection_name, doc_generic, None);
        let indices = meta
            .indices
            .iter()
            .enumerate()
            .map(|(i, v)| v.generate_index_constructor(collection_name, doc_generic, Some(i)));
        quote! {
            #collection_name: #crate_name::Collection::new(
                kv.clone(),
                stringify!(#collection_name).into(),
                #primary_index ,
                vec![
                    #(#indices)*
                ]
            ).await?,
        }
    });

    let collection_names = field_meta
        .iter()
        .map(|(collection_name, _)| collection_name.clone())
        .collect::<Vec<_>>();
    let collection_types = field_meta
        .iter()
        .map(|(_collection_name, meta)| meta.doc_generic.clone())
        .collect::<Vec<_>>();

    // the concrete KV implementation for the database
    let kv_generic_name = get_kv_generic(&input)?;

    let collections_untyped = field_meta.iter().map(|(collection_name, _meta)| {
        quote! {
            Box::new(self. #collection_name .clone()) as Box<dyn #crate_name::AnyCollection<#kv_generic_name>>,
        }
    });

    // the name of the database struct
    let name = &input.ident;
    // the name of the write transaction struct
    let name_write = quote::format_ident!("{}Write", name);
    let result_type = quote::quote! { #crate_name::result::Result };

    // types on the struct
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        pub struct #name_write #impl_generics #where_clause {
            tx: #kv_generic_name::WriteTransaction,
            db: #name #ty_generics,
        }

        impl #impl_generics #name_write #ty_generics #where_clause {
            pub async fn commit(self) -> #result_type<()> {
                self.tx.commit().await
            }

            #(
                pub fn #collection_names(&self) -> #crate_name::CollectionWrite<#collection_types, #kv_generic_name> {
                    #crate_name::CollectionWrite::new(&self.db.#collection_names, &self.tx)
                }
            )*

            pub fn query<'a, T: #crate_name::CollectionDocument + 'static>(&'a self) -> <T as #crate_name::Queryable>::RunnableDocumentQuery<'a, Self>
                    where
                        Self: #crate_name::QueryFor<T, Self> + #crate_name::WriteQueryOperations<T>
                {
                    <Self as #crate_name::QueryFor<T, Self>>::query_for(self)
                }
        }


        #(
            impl #impl_generics #crate_name::WriteQueryOperations<#collection_types> for #name_write #ty_generics #where_clause {
                async fn find_exact(&self, query_doc: <#collection_types as #crate_name::Queryable>::DocumentQuery) -> #crate_name::result::Result<Option<#collection_types>> {
                    #crate_name::CollectionWrite::new(&self.db.#collection_names, &self.tx).find_exact(query_doc).await
                }

                async fn delete_exact(&self, query_doc: <#collection_types as #crate_name::Queryable>::DocumentQuery) -> #crate_name::result::Result<Option<#collection_types>> {
                    #crate_name::CollectionWrite::new(&self.db.#collection_names, &self.tx).delete_exact(query_doc).await
                }

                async fn update_exact(
                    &self,
                    query_doc: <#collection_types as #crate_name::Queryable>::DocumentQuery,
                    update_fn: impl FnOnce(#collection_types) -> #collection_types,
                ) -> #result_type<bool> {
                    #crate_name::CollectionWrite::new(&self.db.#collection_names, &self.tx).update_exact(query_doc, update_fn).await
                }

                async fn update_exact_or_insert(
                    &self,
                    query_doc: <#collection_types as #crate_name::Queryable>::DocumentQuery,
                    update_fn: impl FnOnce(Option<#collection_types>) -> #collection_types,
                ) -> #result_type<()> {
                    #crate_name::CollectionWrite::new(&self.db.#collection_names, &self.tx).update_exact_or_insert(query_doc, update_fn).await
                }
            }

            impl #impl_generics #crate_name::QueryFor<#collection_types, Self> for #name_write #ty_generics #where_clause {
                fn query_for<'a>(&'a self) -> <#collection_types as #crate_name::Queryable>::RunnableDocumentQuery<'a, Self> {
                    <#collection_types as #crate_name::Queryable>::RunnableDocumentQuery::new(&self)
                }
            }
        )*

        const _: fn() = || {
            fn assert_impl<#name: Clone>() {}
        };

        impl #impl_generics #name #ty_generics #where_clause {
            /// Initialize the database backed by a kv, optionally providing a config.
            pub async fn new(config_maybe: Option<#kv_generic_name::Config>) -> #crate_name::result::Result<Self> {
                let kv = #kv_generic_name::new(config_maybe).await?;
                Ok(Self {
                    #(#collections)*
                })
            }

            pub async fn write_tx(&self) -> #crate_name::result::Result<#name_write #ty_generics> {
                Ok(#name_write { tx: self.kv().write_tx().await?, db: self.clone() })
            }

            pub fn kv(&self) -> #kv_generic_name {
                self.collections().next().expect("no collections in database").kv()
            }

            pub fn collections(&self) -> impl Iterator<Item = Box<dyn #crate_name::AnyCollection<#kv_generic_name>>> {
                vec![
                    #(#collections_untyped)*
                ].into_iter()
            }

            pub fn query<'a, T: #crate_name::CollectionDocument + 'static>(&'a self) -> <T as #crate_name::Queryable>::RunnableDocumentQuery<'a, #crate_name::Collection<T,#kv_generic_name>>
                    where
                        Self: #crate_name::QueryFor<T, #crate_name::Collection<T, #kv_generic_name>>
                {
                    <Self as #crate_name::QueryFor<T, #crate_name::Collection<T, K>>>::query_for(self)
                }
        }

        #(
                impl #impl_generics #crate_name::QueryFor<#collection_types, #crate_name::Collection<#collection_types, #kv_generic_name>> for #name #ty_generics #where_clause {
                    fn query_for<'a>(&'a self) -> <#collection_types as #crate_name::Queryable>::RunnableDocumentQuery<'a,#crate_name::Collection<#collection_types, #kv_generic_name>> {
                        <#collection_types as #crate_name::Queryable>::RunnableDocumentQuery::new(&self.#collection_names)
                    }
                }
            )*
    };

    Ok(TokenStream::from(expanded))
}

/// For all the collections in the db, extract primary_key and index attributes.
fn parse_attributes(field: Field) -> Result<(IndexDef, Vec<IndexDef>)> {
    let mut primary_key_maybe: Option<IndexDef> = None;
    let mut indices = Vec::default();
    for attr in field.attrs.iter() {
        if !attr.path().is_ident("anondb") {
            continue;
        }
        let index_def = attr.parse_args::<IndexDef>()?;
        match index_def.index_type {
            IndexType::Primary => {
                if !index_def.options.is_empty() {
                    return Err(Error::new_spanned(
                        attr,
                        "AnonDB primary_key attribute does not support options",
                    ));
                }
                if primary_key_maybe.is_some() {
                    return Err(Error::new(
                        attr.span(),
                        "Primary key may be specified only once!",
                    ));
                }
                primary_key_maybe = Some(index_def);
            }
            IndexType::Normal => {
                indices.push(index_def);
            }
        }
    }
    let primary_key = primary_key_maybe.ok_or_else(|| {
        Error::new_spanned(
            &field,
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

/// Remove any generics on a type
fn assert_no_generics(ty: &Type) -> Result<()> {
    let Type::Path(mut type_path) = ty.clone() else {
        return Err(Error::new(ty.span(), "Expected type to be a path"));
    };

    for segment in &mut type_path.path.segments {
        match segment.arguments {
            PathArguments::None => {}
            _ => {
                return Err(Error::new(
                    segment.span(),
                    "Collection generics may not have generic arguments",
                ));
            }
        }
    }

    Ok(())
}
