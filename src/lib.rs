#![allow(dead_code)]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::collections::HashSet;
use std::iter::FromIterator;
use syn::visit::Visit;
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Field, GenericArgument, Ident, Result, Token,
};

#[derive(Default)]
struct TypeArgumentsCollectorVisitor {
    items: HashSet<String>,
}

impl<'ast> Visit<'ast> for TypeArgumentsCollectorVisitor {
    fn visit_ident(&mut self, id: &'ast Ident) {
        self.items.insert(id.to_string());
    }
}

struct TypeArgumentsCheckVisitor<'a> {
    generics: &'a Vec<(&'a GenericArgument, HashSet<String>)>,
    matched_generics: Vec<&'a (&'a GenericArgument, HashSet<String>)>,
}

impl<'ast> Visit<'ast> for TypeArgumentsCheckVisitor<'ast> {
    fn visit_ident(&mut self, id: &'ast Ident) {
        let name = &id.to_string();
        for g in self.generics.iter() {
            for ident in g.1.iter() {
                if ident == name {
                    self.matched_generics.push(g);
                }
            }
        }
    }
}

struct Generics {
    start: Token![<],
    args: Punctuated<GenericArgument, Token![,]>,
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

struct Action {
    name: Ident,
    parens: token::Paren,
    fields: Punctuated<Ident, Token![,]>,
}

impl Parse for Action {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        Ok(Action {
            name: input.parse()?,
            parens: parenthesized!(content in input),
            fields: content.parse_terminated(Ident::parse)?,
        })
    }
}

struct ConfigurationExpr {
    struct_name: Ident,
    actions: Punctuated<Action, Token![,]>,
}

impl Parse for ConfigurationExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        let struct_content;
        let struct_name = input.parse()?;
        input.parse::<Token![=>]>()?;
        bracketed!(struct_content in input);
        let actions = struct_content.parse_terminated(Action::parse)?;
        Ok(ConfigurationExpr {
            struct_name,
            actions,
        })
    }
}

struct StructGen {
    generics: Generics,
    brace: token::Brace,
    fields: Punctuated<Field, Token![,]>,
    arrow: token::FatArrow,
    conf_brace: token::Brace,
    conf: Punctuated<ConfigurationExpr, Token![,]>,
}

impl Parse for StructGen {
    fn parse(input: ParseStream) -> Result<Self> {
        let struct_content;
        let conf_content;

        Ok(StructGen {
            generics: input.parse()?,
            brace: braced!(struct_content in input),
            fields: struct_content.parse_terminated(Field::parse_named)?,
            arrow: input.parse()?,
            conf_brace: braced!(conf_content in input),
            conf: conf_content.parse_terminated(ConfigurationExpr::parse)?,
        })
    }
}

struct StructOutputConfiguration {
    omitted_fields: HashSet<String>,
}

#[proc_macro]
pub fn gen_generics(input: TokenStream) -> TokenStream {
    let StructGen {
        generics: parsed_generics,
        fields: parsed_fields,
        conf,
        ..
    } = parse_macro_input!(input as StructGen);

    let omit_tag = String::from("omit");

    let structs: Vec<(String, StructOutputConfiguration)> = conf
        .iter()
        .map(|c| {
            let mut omitted_fields = HashSet::<String>::new();

            for a in c.actions.iter() {
                let name: String = a.name.to_string();
                if a.name == omit_tag {
                    for f in a.fields.iter() {
                        omitted_fields.insert(f.to_string());
                    }
                } else {
                    panic!(format!("{} is not a valid action", name));
                }
            }

            (
                c.struct_name.to_string(),
                StructOutputConfiguration { omitted_fields },
            )
        })
        .collect();

    let generics: Vec<(&GenericArgument, HashSet<String>)> = parsed_generics
        .args
        .iter()
        .map(|arg| {
            let mut collector = TypeArgumentsCollectorVisitor {
                ..Default::default()
            };
            collector.visit_generic_argument(arg);

            (arg, collector.items)
        })
        .collect();

    let fields: Vec<(&Field, Vec<&(&GenericArgument, HashSet<String>)>)> = parsed_fields
        .iter()
        .map(|f| {
            let mut collector = TypeArgumentsCheckVisitor {
                generics: &generics,
                matched_generics: Vec::new(),
            };
            collector.visit_type(&f.ty);

            (f, collector.matched_generics)
        })
        .collect();

    let token_streams = structs.iter().map(
        |(struct_name, StructOutputConfiguration { omitted_fields })| {
            let mut used_fields = HashSet::<&Field>::new();
            let mut used_generics = HashSet::<&GenericArgument>::new();

            for (f, gs) in fields.iter() {
                if omitted_fields.contains(&f.ident.as_ref().unwrap().to_string()) {
                    continue;
                }

                used_fields.insert(f);
                used_generics.extend(gs.iter().map(|t| t.0));
            }

            let u_fields = Vec::from_iter(used_fields);
            let u_generics = Vec::from_iter(used_generics);
            let struct_name_ident = Ident::new(struct_name, Span::call_site());

            quote! {
                struct #struct_name_ident <#(#u_generics),*> {
                    #(#u_fields),*
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
                absolute_path("./src/test_fixtures/testbed/Cargo.toml")
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
            use struct_gen::gen_generics;
            struct OnlyBar<T> {
                bar: T,
            }
            struct OnlyFoo {
                foo: u32,
            }
        }
        "###);
    }
}
