use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(FieldTypes)]
pub fn field_types(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(s) => &s.fields,
        _ => panic!("FieldTypes only works on structs"),
    };

    let match_arms: Vec<_> = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| {
                let ident = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                quote! {stringify!(#ident) => Some(stringify!(#ty)),}
            })
            .collect(),
        _ => panic!("FieldTypes needs named fields"),
    };

    quote! {
        // #[automatically_derived]
        impl #impl_generics FieldTypes for #name #ty_generics #where_clause {
            fn field_type<__Documented_T: AsRef<str>>(field_name: __Documented_T) -> Option<&'static str> {
                // use phf;

                // static PHF: phf::Map<&'static str, &'static str> = phf::phf_map! {
                //     #(#phf_match_arms)*
                // };
                // PHF.get(field_name.as_ref()).copied()
                match field_name.as_ref() {
                    #(#match_arms)*
                    _ => None,
                }
            }
        }
    }
    .into()
}
