use std::collections::HashMap;

use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::*;

use anondb_kv::SortDirection;

/// Represents a single field in an index.
#[derive(Clone)]
pub struct IndexField {
    pub name: Ident,
    pub direction: SortDirection,
}

impl Parse for IndexField {
    fn parse(input: ParseStream) -> Result<Self> {
        let direction = if input.peek(Token![-]) {
            input.parse::<Token![-]>()?;
            SortDirection::Desc
        } else {
            SortDirection::Asc
        };

        let name: Ident = input.parse()?;

        Ok(IndexField { name, direction })
    }
}

/// Represents an index for a collection.
#[derive(Clone)]
pub struct IndexDef {
    /// Either "index" or "primary_key"
    pub keyword: Ident,
    pub fields: Vec<IndexField>,
    pub options: HashMap<Ident, bool>,
}

impl Parse for IndexDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let keyword: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let field_list = Punctuated::<IndexField, Token![,]>::parse_separated_nonempty(input)?;
        let fields: Vec<IndexField> = field_list.into_iter().collect();

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
            keyword,
            fields,
            options,
        })
    }
}
