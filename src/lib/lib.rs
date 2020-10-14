extern crate proc_macro;

use proc_macro::TokenStream;
use syn;
use syn::{
    braced, bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, ExprCall, GenericArgument, Ident, Result, Token, Type,
};

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

struct ConfigurationExpr {
    field: Ident,
    actions: Punctuated<ExprCall, Token![,]>,
}

impl Parse for ConfigurationExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        println!("Hi!");
        let struct_content;
        let field = input.parse()?;
        input.parse::<Token![=>]>()?;
        bracketed!(struct_content in input);
        let actions = struct_content.parse_terminated(ExprCall::parse)?;
        Ok(ConfigurationExpr { field, actions })
    }
}

struct Field {
    name: Ident,
    colon_token: Token![:],
    ty: Type,
}

impl Parse for Field {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Field {
            name: input.parse()?,
            colon_token: input.parse()?,
            ty: input.parse()?,
        })
    }
}

struct Defn {
    generics: Generics,
    brace: token::Brace,
    fields: Punctuated<Field, Token![,]>,
    arrow: token::FatArrow,
    conf_brace: token::Brace,
    conf: Punctuated<ConfigurationExpr, Token![,]>,
}

impl Parse for Defn {
    fn parse(input: ParseStream) -> Result<Self> {
        let struct_content;
        let conf_content;

        Ok(Defn {
            generics: input.parse()?,
            brace: braced!(struct_content in input),
            fields: struct_content.parse_terminated(Field::parse)?,
            arrow: input.parse()?,
            conf_brace: braced!(conf_content in input),
            conf: conf_content.parse_terminated(ConfigurationExpr::parse)?,
        })
    }
}

#[proc_macro]
pub fn defn(input: TokenStream) -> TokenStream {
    let defn = parse_macro_input!(input as Defn);

    unimplemented!();
}
