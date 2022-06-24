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

/// used in normal blocks (except ID Block)
#[proc_macro_attribute]
pub fn normal_object_v3(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { pub id:String}));
        fields.named.push(get_field_def(quote! { pub block_size:u64}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

#[proc_macro_attribute]
pub fn normal_object_v4(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { pub id:String}));
        fields.named.push(get_field_def(quote! { pub block_size:u64}));
        fields.named.push(get_field_def(quote! { links_count:u64}));
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
        fields.named.push(get_field_def(quote! { file_id:String}));
        fields.named.push(get_field_def(quote! { spec_type:SpecVer}));
        fields.named.push(get_field_def(quote! { pub unfinalized_flags:Option<UnfinalizedFlagsType>}));
        fields.named.push(get_field_def(quote! { version:u16}));
        fields.named.push(get_field_def(quote! { pub custom_flags:u16}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

/// add BlockId Ref to this block
#[proc_macro_attribute]
pub fn basic_object(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { block_id:Option<BlockId>}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

#[proc_macro_attribute]
pub fn data_group_object(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { pub record_id_type:Option<RecordIDType>}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

#[proc_macro_attribute]
pub fn channel_group_object(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! { pub record_count:u64}));
        fields.named.push(get_field_def(quote! { pub record_size:u32}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

#[proc_macro_attribute]
pub fn channel_object(_args: TokenStream, input:TokenStream) -> TokenStream{
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let _ = parse_macro_input!(_args as parse::Nothing);

    if let syn::Fields::Named(ref mut fields) = item_struct.fields {
        fields.named.push(get_field_def(quote! {
            /// additional offset
            pub add_offset:u32
        }));
        // fields.named.push(get_field_def(quote! { pub bitmask_cache:u64}));
        fields.named.push(get_field_def(quote! { pub bit_offset:u16}));
        fields.named.push(get_field_def(quote! { pub channel_type:Option<ChannelType>}));
        fields.named.push(get_field_def(quote! { pub max_raw:f64}));
        fields.named.push(get_field_def(quote! { pub min_raw:f64}));
        fields.named.push(get_field_def(quote! { pub bits_count:u32}));
        fields.named.push(get_field_def(quote! { pub signal_type:Option<SignalType>}));
        fields.named.push(get_field_def(quote! { pub sync_type:Option<SyncType>}));
        fields.named.push(get_field_def(quote! { pub unit:String}));
    }

    return quote! {
        #item_struct
    }
    .into();
}

#[proc_macro_derive(MDFObject)]
pub fn mdf_object_derive(input:TokenStream) -> TokenStream{
    let ast:DeriveInput = syn::parse(input).unwrap();
    impl_mdf_object_derive(&ast)
}

fn impl_mdf_object_derive(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;
    
    let gen = quote! {
        impl MDFObject for #name {
            fn block_size(&self) -> u64 {
                self.block_size
            }
        
            fn name(&self) -> String {
                self.name.clone()
            }
        }
    };

    gen.into()
}

#[proc_macro_derive(IDObject)]
pub fn id_object_derive(input:TokenStream) -> TokenStream{
    let ast:DeriveInput = syn::parse(input).unwrap();
    impl_id_object_derive(&ast)
}

fn impl_id_object_derive(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;
    
    let gen = quote! {
        impl IDObject for #name {
            /// get the file_id(aka magic number) of IDBlock
            fn file_id(&self) -> String {
                self.file_id.clone()
            }
            /// get specification version of this IDObject
            fn spec_type(&self) -> SpecVer {
                self.spec_type
            }
            /// get unifinalized flags type of this IDObject
            fn unfinalized_flags_type(&self) -> Option<UnfinalizedFlagsType> {
                self.unfinalized_flags
            }
            /// get the version code of this IDObject,
            fn version(&self) -> u16 {
                self.version
            }
        
            fn custom_flags(&self) -> u16 {
                self.custom_flags
            }
        }
    };

    gen.into()
}

#[proc_macro_derive(PermanentBlock)]
pub fn permanent_block_derive(input:TokenStream) -> TokenStream{
    let ast:DeriveInput = syn::parse(input).unwrap();
    impl_permanent_block_derive(&ast)
}

fn impl_permanent_block_derive(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;
    
    let gen = quote! {
        impl PermanentBlock for #name {
        }
    };
    gen.into()
}