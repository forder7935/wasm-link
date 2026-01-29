use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ quote, format_ident };
use syn::{ parse_macro_input, LitStr, Token, punctuated::Punctuated };
use pipe_trait::Pipe ;

/// Generates compile-time constants and lookup functions for fixture IDs.
///
/// Scans the fixture directory for interfaces/ and plugins/ subdirectories,
/// sorts directories alphabetically, and assigns sequential IDs starting from 0.
/// Generates constants named after the directory and lookup functions:
/// ```ignore
/// pub mod interfaces {
///     pub const ROOT: wasm_compose::InterfaceId = wasm_compose::InterfaceId::new(1);
///     pub fn dir_name( id: wasm_compose::InterfaceId ) -> Option<&'static str> { ... }
/// }
/// pub mod plugins {
///     pub const STARTUP: wasm_compose::PluginId = wasm_compose::PluginId::new(1);
///     pub fn dir_name( id: wasm_compose::PluginId ) -> Option<&'static str> { ... }
/// }
/// ```
#[proc_macro]
pub fn generate_fixture_ids( input: TokenStream ) -> TokenStream {

    let segments = parse_macro_input!( input with Punctuated::<LitStr, Token![,]>::parse_terminated );

    let manifest_dir = std::env::var( "CARGO_MANIFEST_DIR" )
        .expect( "CARGO_MANIFEST_DIR not set" );

    let mut fixture_path = std::path::PathBuf::from( &manifest_dir ).join( "tests" );
    segments.iter().for_each(| segment | fixture_path.push( segment.value() ));

    let interface_module = generate_interface_module( &fixture_path );
    let plugin_module = generate_plugin_module( &fixture_path );

    quote! {
        pub mod interfaces {
            #interface_module
        }
        pub mod plugins {
            #plugin_module
        }
    }.into()

}

fn dir_name_to_const_name( dir_name: &str ) -> String {
    dir_name
        .to_uppercase()
        .replace( '-', "_" )
}

fn get_sorted_dirs( parent_dir: &std::path::Path ) -> Vec<String> {

    let entries = match std::fs::read_dir( parent_dir ) {
        Ok( entries ) => entries,
        Err( _ ) => return vec![],
    };

    entries
        .filter_map( Result::ok )
        .filter(| entry | entry.path().is_dir() )
        .filter_map(| entry | entry.file_name().to_str().map( String::from ))
        .collect::<Vec<_>>()
        .pipe(| mut dirs | { dirs.sort(); dirs })

}

fn generate_interface_module( fixture_path: &std::path::Path ) -> TokenStream2 {

    let interfaces_dir = fixture_path.join( "interfaces" );
    let dirs = get_sorted_dirs( &interfaces_dir );

    let consts = dirs.iter()
        .enumerate()
        .map(|( id, dir_name )| {
            let id = id as u64;
            let const_name = format_ident!( "{}", dir_name_to_const_name( dir_name ));
            quote! { pub const #const_name: wasm_compose::InterfaceId = wasm_compose::InterfaceId::new( #id ); }
        })
        .collect::<Vec<_>>();

    let match_arms = dirs.iter()
        .enumerate()
        .map(|( id, dir_name )| {
            let id = id as u64;
            quote! { #id => Some( #dir_name ), }
        })
        .collect::<Vec<_>>();

    quote! {
        #( #consts )*

        pub fn dir_name( id: wasm_compose::InterfaceId ) -> Option<&'static str> {
            match u64::from( id ) {
                #( #match_arms )*
                _ => None,
            }
        }
    }
}

fn generate_plugin_module( fixture_path: &std::path::Path ) -> TokenStream2 {

    let plugins_dir = fixture_path.join( "plugins" );
    let dirs = get_sorted_dirs( &plugins_dir );

    let consts = dirs.iter()
        .enumerate()
        .map(|( id, dir_name )| {
            let id = id as u64;
            let const_name = format_ident!( "{}", dir_name_to_const_name( dir_name ));
            quote! { pub const #const_name: wasm_compose::PluginId = wasm_compose::PluginId::new( #id ); }
        })
        .collect::<Vec<_>>();

    let match_arms: Vec<TokenStream2> = dirs.iter()
        .enumerate()
        .map(|( id, dir_name )| {
            let id = id as u64;
            quote! { #id => Some( #dir_name ), }
        })
        .collect::<Vec<_>>();

    quote! {
        #( #consts )*

        pub fn dir_name( id: wasm_compose::PluginId ) -> Option<&'static str> {
            match u64::from( id ) {
                #( #match_arms )*
                _ => None,
            }
        }
    }
}
