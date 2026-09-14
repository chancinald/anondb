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
    ) -> TokenStream {
        let crate_name = super::crate_name();
        let fields = self.fields.clone();
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

        quote! {
            #crate_name::Index::new(
                stringify!(#collection_name).into(),
                vec![#(
                    (
                        stringify!(#fields).to_string(),
                        <<#doc_generic as #crate_name::Queryable>::DocumentPhantom>::#fields().stats()
                     )
                    ),*
                ],
                |doc: &#doc_generic| -> Vec<u8> {
                    let mut key = #crate_name::LexicographicKey::default();
                    #(
                        let bytes = <_ as #crate_name::SerializeLexicographic>::serialize_lex(&doc.#fields);
                        key.append_key_slice(bytes.as_slice());
                    )*
                    key.take()
                },
                #crate_name::IndexOptions {
                    #(#options)*
                    ..Default::default()
                }
            ),
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
