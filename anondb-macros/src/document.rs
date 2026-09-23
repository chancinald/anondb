use proc_macro::TokenStream;
use quote::quote;
use syn::Error;
use syn::Result;
use syn::parse::Parse;
use syn::spanned::Spanned;

use super::*;

pub struct FieldDef {
    name: Ident,
    ty: Type,
    attrs: FieldAttrs,
}

#[derive(Default)]
pub struct FieldAttrs {
    is_skip: bool,
    default_fn_maybe: Option<String>,
}

impl Parse for FieldAttrs {
    fn parse(input: parse::ParseStream) -> Result<Self> {
        let field_list = Punctuated::<Expr, Token![,]>::parse_separated_nonempty(input)?;
        let mut default_fn_maybe: Option<String> = None;
        let mut is_skip = false;
        for field in field_list.into_iter() {
            match field {
                Expr::Path(name) => {
                    if name.path.is_ident("default") {
                        default_fn_maybe = Some("Default::default".into());
                    } else if name.path.is_ident("skip") {
                        is_skip = true;
                    }
                }
                Expr::Assign(expr) => {
                    let left = match *expr.left {
                        Expr::Path(name) => name
                            .path
                            .get_ident()
                            .cloned()
                            .ok_or(Error::new(name.span(), "expected LHS to be Ident")),
                        _ => Err(Error::new(
                            expr.left.span(),
                            "expected expression lhs to be path",
                        )),
                    }?;
                    if &left.to_string() != "default" {
                        continue;
                    }
                    match *expr.right {
                        Expr::Lit(val) => match val.lit {
                            Lit::Str(v) => {
                                default_fn_maybe = Some(v.value().to_string());
                            }
                            _ => {
                                return Err(Error::new(
                                    val.span(),
                                    "expected expression rhs to be string literal",
                                ));
                            }
                        },
                        _ => {
                            return Err(Error::new(
                                expr.right.span(),
                                "expected expression rhs to be literal",
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            is_skip,
            default_fn_maybe,
        })
    }
}

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let crate_name = crate_name();

    let name = &input.ident;
    let generics = &input.generics;
    let phantom_generics = generics
        .type_params()
        .map(|v| {
            let name = &v.ident;
            quote! { #name }
        })
        .chain(generics.lifetimes().map(|v| quote! { &#v () }))
        .collect::<Vec<_>>();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let fields = parse_struct_and_fields(&input, "Document")?
        .into_iter()
        .map(|field| {
            let mut field_def = FieldDef {
                name: field
                    .ident
                    .clone()
                    .ok_or(Error::new(field.span(), "expected field to be named"))?,
                ty: field.ty.clone(),
                attrs: Default::default(),
            };
            for attr in field.attrs.iter() {
                if !attr.path().is_ident("anondb") {
                    continue;
                }
                field_def.attrs = attr.parse_args::<FieldAttrs>()?;
            }
            if field_def.attrs.is_skip && field_def.attrs.default_fn_maybe.is_none() {
                return Err(Error::new(
                    field.span(),
                    "fields that are skipped must have a default value specified",
                ));
            }
            Ok(field_def)
        })
        .collect::<Result<Vec<FieldDef>>>()?;
    let field_count = fields.len();

    let mut migrations_fn_maybe = None;
    for attr in input.attrs.iter() {
        if !attr.path().is_ident("document") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("migrations") {
                if migrations_fn_maybe.is_some() {
                    return Err(syn::Error::new(
                        meta.path.span(),
                        "duplicate document migrations definition!",
                    ));
                }
                let fn_name = meta.value()?.parse::<syn::Path>()?;
                migrations_fn_maybe = Some(quote! {
                    fn migrations<'a>() -> &'static [fn(
                        Vec<(String, Vec<u8>)>,
                        Vec<String>,
                        Vec<u8>
                    ) ->
                            Result<(Vec<(String, Vec<u8>)>, Vec<String>, Vec<u8>),
                            #crate_name::MigrationError>] {
                        #fn_name ()
                    }
                });
            }
            Ok(())
        })?;
    }

    let skipped_fields = fields.iter().filter(|f| f.attrs.is_skip).map(|f| {
        let name = &f.name;
        let func = f
            .attrs
            .default_fn_maybe
            .as_ref()
            .unwrap_or_else(|| panic!("no default value set for field {}", f.name));
        let expr: Expr = parse_str(func).unwrap();
        quote! {
            #name: #expr(),
        }
    });

    let field_types = fields
        .iter()
        .filter(|f| !f.attrs.is_skip)
        .map(|f| f.ty.clone())
        .collect::<Vec<_>>();

    let field_names = fields
        .iter()
        .filter(|f| !f.attrs.is_skip)
        .map(|f| f.name.clone())
        .collect::<Vec<_>>();

    let field_constraints = fields.iter().filter(|f| !f.attrs.is_skip).map(|f| {
        let field_type = f.ty.clone();
        quote! {
            const _: fn() = || {
                fn assert_valid_type<T: #crate_name::ValidDBType>() {}
                assert_valid_type::<#field_type>();
            };
        }
    });

    let query_struct_name = quote::format_ident!("{}_Query", name);
    let phantom_struct_name = quote::format_ident!("{}_Phantom", name);
    let maybe_struct_name = quote::format_ident!("{}_Maybe", name);

    let expanded = quote! {

        #[derive(Default, Clone, Debug)]
        #[allow(non_camel_case_types)]
        pub struct #query_struct_name #impl_generics #where_clause {
            #(
                pub #field_names : Option<#crate_name::QueryParam<#field_types>>,
            )*
            _phantom: std::marker::PhantomData<(#(#phantom_generics),*)>
        }

        impl #impl_generics #query_struct_name #ty_generics #where_clause {
            #(
                pub fn #field_names (self) -> #crate_name::QueryBuilder<Self, #field_types> {
                    use #crate_name::GeneralQueryBuilder;
                    #crate_name::QueryBuilder::with_query(self, |param, mut this| {
                        if this.#field_names.is_some() {
                            println!("anondb: warning, param already set; {}", stringify!(#field_names));
                        }
                        this.#field_names = Some(param);
                        this
                    })
                }
            )*
        }

        impl #impl_generics #crate_name::Query for #query_struct_name #ty_generics #where_clause {
            /// Get a vector containing the fields that have been set.
            fn serialize(&self) -> Vec<(&'static str, #crate_name::QueryParam<Vec<u8>>)> {
                let mut out = Vec::with_capacity(#field_count);
                #(
                    if let Some(param) = self.#field_names.as_ref() {
                        out.push((stringify!(#field_names), param.serialize()));
                    }
                )*
                out
            }
        }

        #(#field_constraints)*

        #[derive(Debug)]
        #[allow(non_camel_case_types)]
        pub struct #phantom_struct_name;

        impl #phantom_struct_name {
            #(
                pub fn #field_names() -> #crate_name::OwnedLexStats {
                    <#field_types as #crate_name::LexStats>::to_owned()
                }
            )*
        }

        impl #impl_generics #crate_name::Queryable for #name #ty_generics #where_clause {
            type DocumentQuery = #query_struct_name #ty_generics;
            type DocumentPhantom = #phantom_struct_name;

            fn matches(&self, query: &Self::DocumentQuery) -> bool {
                #(
                    if let Some(param) = query.#field_names.as_ref() {
                        if !param.test(&self.#field_names) {
                            return false;
                        }
                    }
                )*
                true
            }
        }

        impl #impl_generics #crate_name::AnyDocument for #name #ty_generics #where_clause {
            fn field_types() -> &'static [(&'static str, &'static str)] {
                &[
                    #(
                        (stringify!(#field_names), stringify!(#field_types)),
                    )*
                ]
            }
        }

        impl #impl_generics #crate_name::Migrateable for #name #ty_generics #where_clause {
            #migrations_fn_maybe
        }

        impl #impl_generics #crate_name::Maybe for #name #ty_generics #where_clause {
            type MaybeStruct = #maybe_struct_name #ty_generics;
        }

        #[derive(Debug, Default)]
        #[allow(non_camel_case_types)]
        pub struct #maybe_struct_name #impl_generics #where_clause {
            #(pub #field_names : Option<#field_types>,)*
            _phantom: std::marker::PhantomData<(#(#phantom_generics),*)>
        }

        impl #impl_generics #maybe_struct_name #ty_generics #where_clause {
            pub(crate) fn serialize_remaining(mut self) -> #crate_name::postcard::Result<Vec<u8>> {
                let mut out = Vec::default();
                #(
                    if let Some(value) = self.#field_names.take() {
                        out = #crate_name::postcard::to_extend(&value, out)?;

                    }
                )*
                Ok(out)
            }

            pub(crate) fn deserialize_remaining(&mut self, mut bytes: &[u8]) -> #crate_name::postcard::Result<()> {
                #(
                    if self.#field_names.is_none() {
                        let (v, rem) = #crate_name::postcard::take_from_bytes::<'_, #field_types>(bytes)?;
                        self.#field_names = Some(v);
                        bytes = rem;
                    }
                )*
                assert!(bytes.is_empty(), "did not parse all bytes");
                Ok(())
            }
        }

        impl #impl_generics From<#name #ty_generics> for #maybe_struct_name #ty_generics #where_clause {
            fn from(value: #name #ty_generics) -> Self {
                Self {
                    #(
                        #field_names: Some(value.#field_names),
                    )*
                    _phantom: std::marker::PhantomData::default()
                }
            }
        }

        impl #impl_generics Into<#name #ty_generics> for #maybe_struct_name #ty_generics #where_clause {
            fn into(mut self) -> #name #ty_generics {
                #name {
                    #(
                        #field_names: self.#field_names.take().expect(""),
                    )*
                    #(
                        #skipped_fields
                    )*
                }
            }
        }
    };

    Ok(TokenStream::from(expanded))
}
