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

impl FieldDef {
    pub fn ty(&self) -> &Type {
        if let Some(try_as_type) = self.attrs.try_as_type_maybe.as_ref() {
            try_as_type
        } else {
            &self.ty
        }
    }
}

#[derive(Default)]
pub struct FieldAttrs {
    is_skip: bool,
    try_as_type_maybe: Option<Type>,
    default_fn_maybe: Option<String>,
}

impl Parse for FieldAttrs {
    fn parse(input: parse::ParseStream) -> Result<Self> {
        let field_list = Punctuated::<Expr, Token![,]>::parse_separated_nonempty(input)?;
        let mut default_fn_maybe: Option<String> = None;
        let mut is_skip = false;
        let mut try_as_type_maybe = None;
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
                    if &left.to_string() == "try_as" {
                        match *expr.right {
                            Expr::Path(val) => {
                                try_as_type_maybe = Some(Type::Path(TypePath {
                                    qself: val.qself.clone(),
                                    path: val.path.clone(),
                                }));
                            }
                            _ => {
                                return Err(Error::new(
                                    expr.right.span(),
                                    "expected expression rhs to be a type",
                                ));
                            }
                        }
                    } else if &left.to_string() == "default" {
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
                }
                _ => {}
            }
        }
        Ok(Self {
            is_skip,
            default_fn_maybe,
            try_as_type_maybe,
        })
    }
}

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    let crate_name = crate_name();

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    // generics for the MaybeStruct type
    let mut maybe_generics = generics.clone();
    let (maybe_impl_generics, maybe_ty_generics, maybe_where_clause) = {
        maybe_generics
            .params
            .insert(0, syn::GenericParam::Lifetime(syn::parse_quote!('__maybe)));
        maybe_generics.split_for_impl()
    };
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
                            #crate_name::result::Result<(Vec<(String, Vec<u8>)>, Vec<String>, Vec<u8>),
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
        .map(|f| f.ty().clone())
        .collect::<Vec<_>>();

    let field_names = fields
        .iter()
        .filter(|f| !f.attrs.is_skip)
        .map(|f| f.name.clone())
        .collect::<Vec<_>>();
    let cast_maybe = fields
        .iter()
        .filter(|f| !f.attrs.is_skip)
        .map(|f| {
            if let Some(_ty) = f.attrs.try_as_type_maybe.as_ref() {
                quote::quote! {
                    .clone().try_into()?
                }
            } else {
                quote::quote! {}
            }
        })
        .collect::<Vec<_>>();
    let try_from_fields = fields
        .iter()
        .filter(|f| !f.attrs.is_skip)
        .map(|f| {
            let name = &f.name;
            if let Some(_ty) = f.attrs.try_as_type_maybe.as_ref() {
                quote! {
                    std::borrow::Cow::Owned(value.#name.clone().try_into()?)
                }
            } else {
                quote! {
                    std::borrow::Cow::Borrowed(&value.#name)
                }
            }
        })
        .collect::<Vec<_>>();

    let field_constraints = fields.iter().filter(|f| !f.attrs.is_skip).map(|f| {
        let field_type = f.ty().clone();
        quote! {
            const _: fn() = || {
                fn assert_valid_type<T: #crate_name::ValidDBType>() {}
                assert_valid_type::<#field_type>();
            };
        }
    });

    let query_struct_name = quote::format_ident!("{}_Query", name);
    let runnable_query_struct_name = quote::format_ident!("{}_RunnableQuery", name);
    let phantom_struct_name = quote::format_ident!("{}_Phantom", name);
    let maybe_struct_name = quote::format_ident!("{}_Maybe", name);
    let fixed_width_getter_names = field_names
        .iter()
        .map(|name| quote::format_ident!("__fixed_width_{}", name))
        .collect::<Vec<_>>();

    let result_type = quote::quote! { #crate_name::result::Result };

    let expanded = quote! {
        #[derive(Debug)]
        #[allow(non_camel_case_types)]
        pub struct #runnable_query_struct_name<'a, P: #crate_name::WriteQueryOperations<#name> + Send + Sync> {
            query: #query_struct_name,
            provider: &'a P,
        }

        impl <'a, P: #crate_name::WriteQueryOperations<#name> + Send + Sync> #crate_name::RunnableQuery<'a, P> for #runnable_query_struct_name<'a, P> {
            fn new(provider: &'a P) -> Self {
                Self {
                    query: Default::default(),
                    provider,
                }
            }
        }

        impl <'a, P: #crate_name::WriteQueryOperations<#name> + Send + Sync> #runnable_query_struct_name<'a, P> {
            #(
                pub fn #field_names (mut self, value: impl Into<#crate_name::QueryParam<#field_types>>) -> Self {
                    self.query = self.query.#field_names(value);
                    self
                }
            )*

            pub async fn find_exact(self
            ) -> #result_type<Option<#name>> {
                self.provider.find_exact(self.query).await
            }
            pub async fn delete_exact(self
            ) -> #result_type<Option<#name>> {
                self.provider.delete_exact(self.query).await
            }
            pub async fn update_exact(self, f: impl FnOnce(#name) -> #name
            ) -> #result_type<bool> {
                self.provider.update_exact(self.query, f).await
            }
        }
        impl <'a, P: #crate_name::QueryOperations<#name> + Send + Sync + 'a> #runnable_query_struct_name<'a, P> {
            pub async fn find_many(
                self,
            ) -> #result_type<impl #crate_name::futures::Stream<Item = #result_type<#name>> + Send + Sync> {
                self.provider.find_many(self.query).await
            }
            pub async fn find_one(
                self,
            ) -> #result_type<Option<#name>> {
                self.provider.find_one(self.query).await
            }
            pub async fn update(
                self,
                update_fn: impl Fn(&#name) -> Option<#name>,
            ) -> #result_type<()> {
                self.provider.update(self.query, update_fn).await
            }
            pub async fn delete(
                self,
                delete_fn: impl Fn(&#name) -> bool,
            ) -> #result_type<()> {
                self.provider.delete(self.query, delete_fn).await
            }
        }

        #[derive(Default, Clone, Debug)]
        #[allow(non_camel_case_types)]
        pub struct #query_struct_name {
            #(
                pub #field_names : Option<#crate_name::QueryParam<#field_types>>
            ),*
        }

        impl #query_struct_name {
            #(
                pub fn #field_names (mut self, value: impl Into<#crate_name::QueryParam<#field_types>>) -> Self {
                    if self.#field_names.is_some() {
                        println!("anondb: warning, param already set: {}", stringify!(#field_names));
                    }
                    self.#field_names = Some(value.into());
                    self
                }
            )*
        }

        impl Into<#crate_name::Query> for #query_struct_name {
            fn into(self) -> #crate_name::Query {
                let mut out = Vec::with_capacity(#field_count);
                #(
                    if let Some(param) = self.#field_names.as_ref() {
                        out.push((stringify!(#field_names), param.serialize()));
                    }
                )*
                #crate_name::Query { fields: out }
            }
        }

        #(#field_constraints)*

        impl #impl_generics #crate_name::CollectionDocument for #name #ty_generics #where_clause {
            type DocumentPhantom = #phantom_struct_name;
            type MaybeStruct<'a> = #maybe_struct_name<'a>;
        }

        #[derive(Debug)]
        #[allow(non_camel_case_types)]
        pub struct #phantom_struct_name;

        impl #phantom_struct_name {
            #(
                pub fn #field_names() -> #crate_name::OwnedLexStats {
                    <#field_types as #crate_name::LexStats>::to_owned()
                }
            )*

            #(
                pub const fn #fixed_width_getter_names() -> Option<u32> {
                    <#field_types as #crate_name::LexStats>::FIXED_WIDTH
                }
            )*
        }

        impl #impl_generics #crate_name::Queryable for #name #ty_generics #where_clause {
            type DocumentQuery = #query_struct_name;
            type RunnableDocumentQuery<'a, P: #crate_name::WriteQueryOperations<Self> + Send + Sync + 'a> = #runnable_query_struct_name<'a, P>;

            fn matches(&self, query: &Self::DocumentQuery) -> #result_type <bool> {
                #(
                    if let Some(param) = query.#field_names.as_ref() {
                        if !param.test(&(self.#field_names #cast_maybe)) {
                            return Ok(false);
                        }
                    }
                )*
                Ok(true)
            }
        }

        impl #impl_generics #crate_name::ParseableDocument for #name #ty_generics #where_clause {
            fn into_map(self) -> #result_type <std::collections::BTreeMap<String, (String, Box<dyn #crate_name::ValidDBType>)>> {
                let mut out = std::collections::BTreeMap::default();
                #(
                    out.insert(
                        stringify!(#field_names).to_string(),
                        (
                            <#field_types as #crate_name::ValidDBType>::schema_type_id().to_string(),
                            Box::new({
                                let v: #field_types = self.#field_names #cast_maybe ;
                                v
                            }) as Box<dyn #crate_name::ValidDBType>
                        )
                    );
                )*
                Ok(out)
            }
        }

        impl #impl_generics #crate_name::AnyDocument for #name #ty_generics #where_clause {
            fn field_types() -> &'static [(&'static str, &'static str)] {
                use std::sync::LazyLock;
                static V: LazyLock<Vec<(&'static str, &'static str)>> =
                    LazyLock::new(|| vec![
                        #(
                            (stringify!(#field_names), <#field_types as #crate_name::ValidDBType>::schema_type_id()),
                        )*
                    ]);
                V.as_ref()
            }
        }

        impl #impl_generics #crate_name::Migrateable for #name #ty_generics #where_clause {
            #migrations_fn_maybe
        }

        #[derive(Debug, Default)]
        #[allow(non_camel_case_types)]
        pub struct #maybe_struct_name #maybe_impl_generics {
            #(pub #field_names : Option<std::borrow::Cow<'__maybe, #field_types>>),*
        }

        impl #maybe_impl_generics #maybe_struct_name #maybe_ty_generics #maybe_where_clause {
            #(
                /// used to get a strongly typed borrow to the field internally
                /// routing through Cow::borrow
                pub fn #field_names(&self) -> Option<&#field_types> {
                    use std::borrow::Borrow;
                    self.#field_names.as_ref().map(|v| v.borrow())
                }
            )*

        }

        impl #maybe_impl_generics #crate_name::MaybeSer for #maybe_struct_name #maybe_ty_generics #maybe_where_clause {
            fn serialize_remaining(mut self) -> #crate_name::postcard::Result<Vec<u8>> {
                let mut out = Vec::default();
                use std::borrow::Borrow;
                #(
                    if let Some(value) = self.#field_names.as_ref() {
                        out = #crate_name::postcard::to_extend(value.borrow() as &#field_types, out)?;

                    }
                )*
                Ok(out)
            }

            fn deserialize_remaining(&mut self, mut bytes: &[u8]) -> #crate_name::postcard::Result<()> {
                #(
                    if self.#field_names.is_none() {
                        let (v, rem) = #crate_name::postcard::take_from_bytes::<'_, #field_types>(bytes)?;
                        self.#field_names = Some(std::borrow::Cow::Owned(v));
                        bytes = rem;
                    }
                )*
                assert!(bytes.is_empty(), "did not parse all bytes");
                Ok(())
            }

            fn serialize_remaining_to_vec(mut self) -> Vec<(String, String, Box<dyn #crate_name::ValidDBType>)> {
                let mut out = Vec::default();
                #(
                    if let Some(value) = self.#field_names.take() {
                        out.push((
                            stringify!(#field_names).to_string(),
                            <#field_types as #crate_name::ValidDBType>::schema_type_id().into(),
                            Box::new(value.into_owned()) as Box<dyn #crate_name::ValidDBType>
                        ));

                    }
                )*
                out
            }
        }


        impl #maybe_impl_generics TryFrom<&'__maybe #name #ty_generics> for #maybe_struct_name #maybe_ty_generics #maybe_where_clause {
            type Error = #crate_name::error::Error;
            fn try_from(value: &'__maybe #name #ty_generics) -> std::result::Result<#maybe_struct_name #maybe_ty_generics, Self::Error> {
                Ok(Self {
                    #(
                        #field_names: Some(
                            #try_from_fields
                        ),
                    )*
                })
            }
        }

        impl #maybe_impl_generics TryInto<#name #ty_generics> for #maybe_struct_name #maybe_ty_generics #maybe_where_clause {
            type Error = #crate_name::error::Error;
            fn try_into(mut self) -> std::result::Result<#name #ty_generics, Self::Error> {
                Ok(#name {
                    #(
                        #field_names: self.#field_names.take().ok_or(#crate_name::error::anyhow::anyhow!("expected field \"{}\" to be set", stringify!(#field_names)))?.into_owned() #cast_maybe,
                    )*
                    #(
                        #skipped_fields
                    )*
                })
            }
        }
    };

    Ok(TokenStream::from(expanded))
}
