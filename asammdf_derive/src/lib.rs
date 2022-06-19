use proc_macro::TokenStream;
use quote::quote;
use syn::{self, DeriveInput, parse_macro_input,ItemStruct, parse::{self, Parser}, Field};
use syn::__private::TokenStream2;

fn get_field_def(tok:TokenStream2)-> Field{
    syn::Field::parse_named
    .parse2(tok)
    .unwrap()
}

#[proc_macro_attribute]
pub fn mdf_object(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { pub block_size:u64}));
        fields.named.push(get_field_def(quote! { pub name:String}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

#[proc_macro_attribute]
pub fn comment_object(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { pub comment:String}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

#[proc_macro_attribute]
pub fn id_object(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { pub file_id:String}));
        fields.named.push(get_field_def(quote! { pub spec_type:Option<SpecVer>}));
        fields.named.push(get_field_def(quote! { pub unfinalized_flags:Option<UnfinalizedFlagsType>}));
        fields.named.push(get_field_def(quote! { pub version:u16}));
        fields.named.push(get_field_def(quote! { pub custom_flags:u16}));
    }

    return quote! {
        #item_struct
    }
    .into();
}