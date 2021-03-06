#![allow(clippy::eval_order_dependence)]
extern crate proc_macro;

// LinkedHashSet is used instead of HashSet in order to insertion order across the board
use linked_hash_set::LinkedHashSet;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::iter::FromIterator;
use syn::visit::Visit;
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Attribute, Field, GenericArgument, Ident, Result, Token, Type, Visibility, WhereClause,
    WherePredicate,
};

#[derive(Default)]
struct TypeArgumentsCollectorVisitor {
    items: LinkedHashSet<String>,
}

impl<'ast> Visit<'ast> for TypeArgumentsCollectorVisitor {
    fn visit_ident(&mut self, id: &'ast Ident) {
        self.items.insert(id.to_string());
    }
}

struct TypeArgumentsCheckVisitor<'ast> {
    args: &'ast Vec<TypeArgumentConfiguration<'ast>>,
    matched: Vec<&'ast TypeArgumentConfiguration<'ast>>,
}

impl<'ast> Visit<'ast> for TypeArgumentsCheckVisitor<'ast> {
    fn visit_ident(&mut self, id: &'ast Ident) {
        let name = &id.to_string();
        for arg in self.args.iter() {
            for id in arg.identifiers.iter() {
                if id == name {
                    self.matched.push(arg);
                }
            }
        }
    }
}

struct Generics {
    #[allow(dead_code)]
    start: Token![<],
    args: Punctuated<GenericArgument, Token![,]>,
    #[allow(dead_code)]
    end: Token![>],
}

impl Parse for Generics {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Generics {
            start: input.parse()?,
            args: {
                let mut args = Punctuated::new();
                loop {
                    if input.peek(Token![>]) {
                        break;
                    }
                    let value = input.parse()?;
                    args.push_value(value);
                    if input.peek(Token![>]) {
                        break;
                    }
                    let punct = input.parse()?;
                    args.push_punct(punct);
                }
                args
            },
            end: input.parse()?,
        })
    }
}

enum ActionVariant {
    Omit(Punctuated<Ident, Token![,]>),
    Include(Punctuated<Ident, Token![,]>),
    Attr(Punctuated<Attribute, Token![,]>),
    Upsert(Punctuated<Field, Token![,]>),
    AsTuple,
}

struct Action {
    #[allow(dead_code)]
    parens: token::Paren,
    fields: ActionVariant,
}

impl Parse for Action {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let name: Ident = input.parse()?;
        let name_str = &name.to_string();

        Ok(Action {
            parens: parenthesized!(content in input),
            fields: {
                if name_str == "omit" {
                    ActionVariant::Omit(content.parse_terminated(Ident::parse)?)
                } else if name_str == "include" {
                    ActionVariant::Include(content.parse_terminated(Ident::parse)?)
                } else if name_str == "as_tuple" {
                    ActionVariant::AsTuple
                } else if name_str == "attr" {
                    use syn::parse_quote::ParseQuote;
                    ActionVariant::Attr(content.parse_terminated(Attribute::parse)?)
                } else if name_str == "upsert" {
                    ActionVariant::Upsert(content.parse_terminated(Field::parse_named)?)
                } else {
                    panic!("{} is not a valid action", name_str)
                }
            },
        })
    }
}

struct ConfigurationExpr {
    struct_name: Ident,
    #[allow(dead_code)]
    arrow: Token![=>],
    #[allow(dead_code)]
    bracket: token::Bracket,
    actions: Punctuated<Action, Token![,]>,
}

impl Parse for ConfigurationExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        let struct_content;

        Ok(ConfigurationExpr {
            struct_name: input.parse()?,
            arrow: input.parse::<Token![=>]>()?,
            bracket: bracketed!(struct_content in input),
            actions: struct_content.parse_terminated(Action::parse)?,
        })
    }
}

struct StructGen {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    generics: Option<Generics>,
    where_clause: Option<WhereClause>,
    #[allow(dead_code)]
    brace: token::Brace,
    fields: Punctuated<Field, Token![,]>,
    #[allow(dead_code)]
    arrow: token::FatArrow,
    #[allow(dead_code)]
    conf_brace: token::Brace,
    conf: Punctuated<ConfigurationExpr, Token![,]>,
}

impl Parse for StructGen {
    fn parse(input: ParseStream) -> Result<Self> {
        let struct_content;
        let conf_content;

        Ok(StructGen {
            attrs: input.call(Attribute::parse_outer)?,
            visibility: {
                if input.lookahead1().peek(Token![pub]) {
                    Some(input.parse()?)
                } else {
                    None
                }
            },
            generics: {
                if input.lookahead1().peek(Token![<]) {
                    Some(input.parse()?)
                } else {
                    None
                }
            },
            where_clause: {
                if input.lookahead1().peek(Token![where]) {
                    Some(input.parse()?)
                } else {
                    None
                }
            },
            brace: braced!(struct_content in input),
            fields: struct_content.parse_terminated(Field::parse_named)?,
            arrow: input.parse()?,
            conf_brace: braced!(conf_content in input),
            conf: conf_content.parse_terminated(ConfigurationExpr::parse)?,
        })
    }
}

struct StructOutputConfiguration<'ast> {
    omitted_fields: LinkedHashSet<String>,
    included_fields: LinkedHashSet<String>,
    upsert_fields_names: LinkedHashSet<String>,
    upsert_fields: Vec<&'ast Field>,
    attributes: Vec<&'ast Attribute>,
    is_tuple: bool,
}

struct TypeArgumentConfiguration<'ast> {
    arg: &'ast GenericArgument,
    identifiers: LinkedHashSet<String>,
}

#[proc_macro]
pub fn generate(input: TokenStream) -> TokenStream {
    let StructGen {
        attrs: top_level_attrs,
        generics: parsed_generics,
        where_clause,
        fields: parsed_fields,
        conf,
        visibility,
        ..
    } = parse_macro_input!(input as StructGen);

    let structs: Vec<(String, StructOutputConfiguration)> = conf
        .iter()
        .map(|c| {
            let mut omitted_fields = LinkedHashSet::<String>::new();
            let mut included_fields = LinkedHashSet::<String>::new();
            let mut upsert_fields = Vec::<&Field>::new();
            let mut upsert_fields_names = LinkedHashSet::<String>::new();
            let mut attributes = Vec::<&Attribute>::new();
            attributes.extend(top_level_attrs.iter());
            let mut is_tuple = false;

            for a in c.actions.iter() {
                match &a.fields {
                    ActionVariant::Omit(fields) => {
                        omitted_fields.extend(fields.iter().map(|f| f.to_string()));
                    }
                    ActionVariant::Include(fields) => {
                        included_fields.extend(fields.iter().map(|f| f.to_string()));
                    }
                    ActionVariant::Attr(attrs) => {
                        attributes.extend(attrs.iter());
                    }
                    ActionVariant::Upsert(fields) => {
                        upsert_fields_names
                            .extend(fields.iter().map(|f| f.ident.as_ref().unwrap().to_string()));
                        upsert_fields.extend(fields);
                    }
                    ActionVariant::AsTuple => {
                        is_tuple = true;
                    }
                }
            }

            (
                c.struct_name.to_string(),
                StructOutputConfiguration {
                    omitted_fields,
                    included_fields,
                    upsert_fields,
                    upsert_fields_names,
                    attributes,
                    is_tuple,
                },
            )
        })
        .collect();

    let generics: Vec<TypeArgumentConfiguration> = if parsed_generics.is_some() {
        parsed_generics
            .as_ref()
            .unwrap()
            .args
            .iter()
            .map(|arg| {
                let mut collector = TypeArgumentsCollectorVisitor {
                    ..Default::default()
                };
                collector.visit_generic_argument(arg);

                TypeArgumentConfiguration {
                    arg,
                    identifiers: collector.items,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let wheres: Vec<(&WherePredicate, Vec<&TypeArgumentConfiguration>)> = if where_clause.is_some()
    {
        where_clause
            .as_ref()
            .unwrap()
            .predicates
            .iter()
            .map(|p| {
                let mut collector = TypeArgumentsCheckVisitor {
                    args: &generics,
                    matched: Vec::new(),
                };
                collector.visit_where_predicate(&p);

                (p, collector.matched)
            })
            .collect()
    } else {
        Vec::new()
    };

    let fields: Vec<(&Field, Vec<&TypeArgumentConfiguration>)> = parsed_fields
        .iter()
        .map(|f| {
            let mut collector = TypeArgumentsCheckVisitor {
                args: &generics,
                matched: Vec::new(),
            };
            collector.visit_type(&f.ty);

            (f, collector.matched)
        })
        .collect();

    let token_streams = structs.iter().map(
        |(
            struct_name,
            StructOutputConfiguration {
                omitted_fields,
                attributes,
                included_fields,
                upsert_fields,
                upsert_fields_names,
                is_tuple
            },
        )| {
            let mut used_fields = LinkedHashSet::<&Field>::new();
            let mut used_types = LinkedHashSet::<&Type>::new();
            let mut used_generics = LinkedHashSet::<&GenericArgument>::new();
            let mut used_wheres = LinkedHashSet::<&WherePredicate>::new();

            let test_skip_predicate: Box<dyn Fn(&Field) -> bool> = if included_fields.is_empty() {
                Box::new(|f: &Field| {
                    let name = &f.ident.as_ref().unwrap().to_string();
                    upsert_fields_names.contains(name) || omitted_fields.contains(name)
                })
            } else {
                Box::new(|f: &Field| {
                    let name = &f.ident.as_ref().unwrap().to_string();
                    upsert_fields_names.contains(name) || !included_fields.contains(name)
                })
            };

            for (f, type_args) in fields.iter() {
                if test_skip_predicate(f) {
                    continue;
                }

                if *is_tuple {
                    used_types.insert(&f.ty);
                } else {
                    used_fields.insert(f);
                }

                for type_arg in type_args.iter() {
                    used_generics.insert(type_arg.arg);

                    for w in wheres.iter() {
                        for w_type_arg in w.1.iter() {
                            if w_type_arg.arg == type_arg.arg {
                                used_wheres.insert(w.0);
                            }
                        }
                    }
                }
            }
            if *is_tuple {
                used_types.extend(upsert_fields.iter().map(|f| &f.ty));
            } else {
                used_fields.extend(upsert_fields.iter());
            }

            let field_items = Vec::from_iter(used_fields);
            let type_items = Vec::from_iter(used_types);
            let generic_items = Vec::from_iter(used_generics);
            let where_items = Vec::from_iter(used_wheres);
            let struct_name_ident = Ident::new(struct_name, Span::call_site());
            if *is_tuple {
                if where_items.is_empty() {
                    quote! {
                        #(#attributes)*
                        #visibility struct #struct_name_ident <#(#generic_items),*> (#(#type_items),*);
                    }
                } else {
                    quote! {
                        #(#attributes)*
                        #visibility struct #struct_name_ident <#(#generic_items),*> (#(#type_items),*) where #(#where_items),*;
                    }
                }
            } else if where_items.is_empty() {
                quote! {
                    #(#attributes)*
                    #visibility struct #struct_name_ident <#(#generic_items),*> {
                        #(#field_items),*
                    }
                }
            } else {
                quote! {
                    #(#attributes)*
                    #visibility struct #struct_name_ident <#(#generic_items),*> where #(#where_items),* {
                        #(#field_items),*
                    }
                }
            }
        },
    );

    (quote! {
       #(#token_streams)*
    })
    .into()
}

#[cfg(test)]
mod tests {
    use path_clean::PathClean;
    use std::env;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub fn absolute_path(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        let path = path.as_ref();

        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir()?.join(path)
        }
        .clean();

        Ok(absolute_path)
    }

    fn run_for_fixture(fixture: &str) -> String {
        let output = Command::new("cargo")
            .arg("expand")
            .arg(fixture)
            .arg("--manifest-path")
            .arg(format!(
                "{}",
                absolute_path("./test_fixtures/testbed/Cargo.toml")
                    .unwrap()
                    .display()
            ))
            .output()
            .expect("Failed to spawn process");

        String::from_utf8_lossy(&output.stdout)
            .to_owned()
            .to_string()
    }

    #[test]
    fn generics() {
        insta::assert_snapshot!(run_for_fixture("generics"), @r###"
        pub mod generics {
            use structout::generate;
            struct OnlyBar<T> {
                bar: T,
            }
            struct OnlyFoo {
                foo: u32,
            }
        }
        "###);
    }

    #[test]
    fn wheres() {
        insta::assert_snapshot!(run_for_fixture("wheres"), @r###"
        pub mod wheres {
            use structout::generate;
            struct OnlyBar<C>
            where
                C: Copy,
            {
                bar: C,
            }
            struct OnlyFoo<S>
            where
                S: Sized,
            {
                foo: S,
            }
        }
        "###);
    }

    #[test]
    fn simple() {
        insta::assert_snapshot!(run_for_fixture("simple"), @r###"
        pub mod simple {
            use structout::generate;
            struct WithoutFoo {
                bar: u64,
                baz: String,
            }
            struct WithoutBar {
                foo: u32,
                baz: String,
            }
            # [object (context = Database)]
            #[object(config = "latest")]
            struct WithAttrs {
                foo: u32,
                bar: u64,
                baz: String,
            }
        }
        "###);
    }

    #[test]
    fn visibility() {
        insta::assert_snapshot!(run_for_fixture("visibility"), @r###"
        pub mod visibility {
            use structout::generate;
            pub(crate) struct Everything {
                foo: u32,
            }
        }
        "###);
    }

    #[test]
    fn include() {
        insta::assert_snapshot!(run_for_fixture("include"), @r###"
        pub mod include {
            use structout::generate;
            struct WithoutFoo {
                bar: u64,
            }
            struct WithoutBar {
                foo: u32,
            }
        }
        "###);
    }

    #[test]
    fn as_tuple() {
        insta::assert_snapshot!(run_for_fixture("as_tuple"), @r###"
        pub mod as_tuple {
            use structout::generate;
            struct OnlyBar<C>(C, i32)
            where
                C: Copy;
            struct OnlyFoo<S>(S, i32)
            where
                S: Sized;
        }
        "###);
    }

    #[test]
    fn upsert() {
        insta::assert_snapshot!(run_for_fixture("upsert"), @r###"
        pub mod upsert {
            use structout::generate;
            struct NewFields {
                foo: u32,
                bar: i32,
                baz: i64,
            }
            struct OverriddenField {
                foo: u64,
            }
            struct Tupled(u64);
        }
        
        "###);
    }

    #[test]
    fn shared_attrs() {
        insta::assert_snapshot!(run_for_fixture("shared_attrs"), @r###"
        pub mod shared_attrs {
            use structout::generate;
            struct InheritsAttributes {
                foo: u32,
            }
            #[automatically_derived]
            #[allow(unused_qualifications)]
            impl ::core::fmt::Debug for InheritsAttributes {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    match *self {
                        InheritsAttributes {
                            foo: ref __self_0_0,
                        } => {
                            let mut debug_trait_builder = f.debug_struct("InheritsAttributes");
                            let _ = debug_trait_builder.field("foo", &&(*__self_0_0));
                            debug_trait_builder.finish()
                        }
                    }
                }
            }
            struct InheritsAttributesTwo {
                foo: u32,
            }
            #[automatically_derived]
            #[allow(unused_qualifications)]
            impl ::core::fmt::Debug for InheritsAttributesTwo {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    match *self {
                        InheritsAttributesTwo {
                            foo: ref __self_0_0,
                        } => {
                            let mut debug_trait_builder = f.debug_struct("InheritsAttributesTwo");
                            let _ = debug_trait_builder.field("foo", &&(*__self_0_0));
                            debug_trait_builder.finish()
                        }
                    }
                }
            }
        }
        "###);
    }
}
