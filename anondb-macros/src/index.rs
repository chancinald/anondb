use std::collections::HashMap;
use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::*;

#[derive(Clone, PartialEq)]
pub enum IndexType {
    Primary,
    Normal,
}

/// Represents an index for a collection as parsed from the DB struct.
#[derive(Clone)]
pub struct IndexDef {
    /// Either "index" or "primary_key"
    pub index_type: IndexType,
    /// Field names in the index
    pub fields: Vec<Ident>,
    pub options: HashMap<Ident, bool>,
}

impl IndexDef {
    pub(crate) fn generate_index_constructor(
        &self,
        collection_name: &Ident,
        doc_generic: &Type,
        index_num_maybe: Option<usize>,
    ) -> TokenStream {
        let crate_name = super::crate_name();
        let fields = self.fields.clone();
        let is_last_field = self
            .fields
            .iter()
            .enumerate()
            .map(|(i, _)| i == self.fields.len() - 1)
            .collect::<Vec<_>>();
        let field_width_getters = self
            .fields
            .iter()
            .map(|name| quote::format_ident!("__fixed_width_{}", name))
            .collect::<Vec<_>>();
        let field_width_errors = self
            .fields
            .iter()
            .map(|name| format!(
                    "In collection \"{}\", {} expected field \"{}\" to be a constant width type. Only the final component of an index may be variable width.",
                    collection_name,
                    if let Some(index_num) = index_num_maybe {
                        format!("index #{index_num}")
                    } else {
                        format!("primary key")
                    },
                    name.to_string()
                    )
                )
            .collect::<Vec<_>>();
        let mut options = self
            .options
            .iter()
            .map(|(k, v)| {
                quote! {
                    #k : #v,
                }
            })
            .collect::<Vec<_>>();
        if self.index_type == IndexType::Primary {
            options.push(quote! {
                unique: true,
            });
        }

        match self.index_type {
            IndexType::Primary => {
                quote! {
                        {
                            type DocumentPhantom = <#doc_generic as #crate_name::CollectionDocument>::DocumentPhantom;
                            type MaybeStruct<'a> = <#doc_generic as #crate_name::CollectionDocument>::MaybeStruct<'a>;
                            #(
                                const _: () = assert!(
                                    #is_last_field ||
                                    DocumentPhantom::#field_width_getters().is_some(),
                                    #field_width_errors
                                );
                            )*
                    #crate_name::PrimaryIndex::new(
                        vec![#(
                            (
                                stringify!(#fields).to_string(),
                                DocumentPhantom::#fields()
                             )
                            ),*
                        ],
                        |doc: &#doc_generic, full_doc: bool| -> #crate_name::result::Result<(Vec<u8>, Option<Vec<u8>>)> {
                            let mut maybe_struct = MaybeStruct::try_from(doc)?;
                            let mut key = #crate_name::LexicographicKey::default();
                            #(
                                {
                                    key.append(maybe_struct.#fields().expect("field already taken"));
                                    maybe_struct.#fields = None;
                                    assert!(
                                        #is_last_field || DocumentPhantom::#fields().fixed_width.is_some(),
                                        #field_width_errors
                                    );
                                }
                            )*
                            Ok(if full_doc {
                                (key.into_vec(), Some(maybe_struct.serialize_remaining()?))
                            } else {
                                (key.into_vec(), None)
                            })
                        },
                        |mut key: &[u8]| -> #crate_name::result::Result<MaybeStruct<'_>> {
                            let mut maybe_struct = MaybeStruct::default();
                            #(
                                {
                                    let len_maybe = if #is_last_field &&
                                        DocumentPhantom::#fields().fixed_width.is_none()
                                    {
                                        Some(key.len())
                                    } else {
                                        assert!(DocumentPhantom::#fields().fixed_width.is_some());
                                        None
                                    };
                                    assert!(maybe_struct.#fields.is_none(), "field aleady parsed");
                                    let (v, rem) = #crate_name::DeserializeLexicographic::deserialize(key, len_maybe)?;
                                    maybe_struct.#fields = Some(std::borrow::Cow::Owned(v));
                                    if !rem.is_empty() {
                                        // skip 1 byte separator in the lexicographic key
                                        let (sep, rem) = rem.split_at(1);
                                        assert_eq!(sep[0], 0);
                                        key = rem;
                                    } else {
                                        key = rem;
                                    }
                                }
                            )*
                            assert!(key.is_empty(), "did not parse all key elements, {} remaining", key.len());
                            Ok(maybe_struct)
                        }
                    )
                }
                }
            }
            IndexType::Normal => {
                quote! {
                    {
                            type DocumentPhantom = <#doc_generic as #crate_name::CollectionDocument>::DocumentPhantom;
                            type MaybeStruct<'a> = <#doc_generic as #crate_name::CollectionDocument>::MaybeStruct<'a>;
                        #(
                            const _: () = assert!(
                                #is_last_field ||
                                DocumentPhantom::#field_width_getters().is_some(),
                                #field_width_errors
                            );
                        )*
                    #crate_name::Index::new(
                        stringify!(#collection_name).into(),
                        vec![#(
                            (
                                stringify!(#fields).to_string(),
                                DocumentPhantom::#fields()
                             )
                            ),*
                        ],
                        |doc: &#doc_generic| -> #crate_name::result::Result<Vec<u8>> {
                            let mut maybe_struct = MaybeStruct::try_from(doc)?;
                            let mut key = #crate_name::LexicographicKey::default();
                            #(
                                key.append(maybe_struct.#fields().unwrap());
                            )*
                            Ok(key.into_vec())
                        },
                        |mut key: &[u8]| -> #crate_name::result::Result<MaybeStruct<'_>> {
                            let mut maybe_struct = MaybeStruct::default();
                            #(
                                {
                                    let len_maybe = if #is_last_field &&
                                        DocumentPhantom::#fields().fixed_width.is_none()
                                    {
                                        Some(key.len())
                                    } else {
                                        assert!(DocumentPhantom::#fields().fixed_width.is_some());
                                        None
                                    };
                                    assert!(maybe_struct.#fields.is_none(), "field aleady parsed");
                                    let (v, rem) = #crate_name::DeserializeLexicographic::deserialize(key, len_maybe)?;
                                    maybe_struct.#fields = Some(std::borrow::Cow::Owned(v));
                                    if !rem.is_empty() {
                                        // skip 1 byte separator in the lexicographic key
                                        let (sep, rem) = rem.split_at(1);
                                        assert_eq!(sep[0], 0);
                                        key = rem;
                                    } else {
                                        key = rem;
                                    }
                                }
                            )*
                            assert!(key.is_empty(), "did not parse all key elements, {} remaining", key.len());
                            Ok(maybe_struct)
                        },
                        #crate_name::IndexOptions {
                            #(#options)*
                            ..Default::default()
                        }
                    )
                },
                }
            }
        }
    }
}

impl Parse for IndexDef {
    fn parse(input: ParseStream) -> Result<Self> {
        // #[anondb(primary_key = id0, id1, id2)]
        let keyword: Ident = input.parse()?;
        let index_type = match keyword.to_string().as_str() {
            "primary_key" => IndexType::Primary,
            "index" => IndexType::Normal,
            _ => {
                return Err(syn::Error::new(
                    keyword.span(),
                    format!(
                        "bad AnonDB keyword, expected \"primary_key\" or \"index\", got: \"{keyword}\""
                    ),
                ));
            }
        };
        input.parse::<Token![=]>()?;
        let field_list = Punctuated::<Ident, Token![,]>::parse_separated_nonempty(input)?;
        let fields: Vec<Ident> = field_list.into_iter().collect();
        // check that no field is repeated
        {
            let mut fields_set = HashSet::<String>::default();
            for field in fields.iter() {
                if !fields_set.insert(field.to_string()) {
                    return Err(syn::Error::new(
                        field.span(),
                        format!("index contains duplicate field: {}", field),
                    ));
                }
            }
        }

        let mut options = HashMap::default();

        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;

            // Parse comma-separated key=value pairs
            let option_list =
                Punctuated::<(Ident, LitBool), Token![,]>::parse_separated_nonempty_with(
                    input,
                    |input| {
                        let key: Ident = input.parse()?;
                        input.parse::<Token![=]>()?;
                        let value: LitBool = input.parse()?;
                        Ok((key, value))
                    },
                )?;

            for (key, value) in option_list {
                options.insert(key, value.value);
            }
        }
        Ok(Self {
            index_type,
            fields,
            options,
        })
    }
}
