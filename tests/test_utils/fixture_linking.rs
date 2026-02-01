#[macro_export]
macro_rules! fixtures {
    {
        const ROOT          = $root:literal ;
        const INTERFACES    = [ $($interface:literal),* $(,)? ] ;
        const PLUGINS       = [ $($plugin:literal),* $(,)? ] ;
    } => { mod fixtures {

        const FIXTURES_DIR: &'static str = strip_rs( file!() );

        pub const ROOT: &'static str = $root ;
        pub static INTERFACES: std::sync::LazyLock<Vec<InterfaceDir>> =
            std::sync::LazyLock::new(|| vec![ $(
                InterfaceDir::new( $interface )
                    .expect( format!( "Interface {} failed to initialise", $interface ).as_str())
            ),* ]);
        pub static PLUGINS: std::sync::LazyLock<Vec<PluginDir>> =
            std::sync::LazyLock::new(|| vec![ $(
                PluginDir::new( $plugin )
                    .expect( format!( "Plugin {} failed to initialise", $plugin ).as_str())
            ),* ]);

        const fn strip_rs( path: &'static str ) -> &'static str {
            match path.as_bytes() {
                [rest @ .., b'.', b'r', b's'] => {
                    // SAFETY: we just checked that the last three bytes are ".rs",
                    // so the split is at a UTF-8 boundary.
                    unsafe { core::str::from_utf8_unchecked(rest) }
                }
                _ => unreachable!(),
            }
        }

        #[derive( Debug, thiserror::Error )]
        pub enum FixtureError {
            #[error( "IO error: {0}" )] Io( #[from] std::io::Error ),
            #[error( "TOML parse error: {0}" )] Toml( #[from] toml::de::Error ),
            #[error( "WIT parser error: {0}" )] WitParser( #[from] anyhow::Error ),
            #[error( "No root interface found" )] NoRootInterface,
            #[error( "No package for root interface" )] NoPackage,
            #[error( "Undeclared type: {0:?}" )] UndeclaredType( wit_parser::TypeId ),
            #[error( "WASM load error: {0}" )] WasmLoad( String ),
        }

        #[derive( Debug, Clone )]
        pub struct InterfaceDir {
            id: String,
            cardinality: wasm_link::InterfaceCardinality,
            wit_data: InterfaceWitData,
        }

        impl InterfaceDir {

            pub fn new( id: &'static str ) -> Result<Self, FixtureError> {

                let root_path = std::path::PathBuf::from( FIXTURES_DIR ).join( "interfaces" ).join( id );
                let manifest_path = root_path.join( "manifest.toml" );
                let manifest_data: InterfaceManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;
                let cardinality = manifest_data.cardinality.into();

                let wit_data = parse_wit( &root_path )?;

                Ok( Self { id: id.to_string(), cardinality, wit_data })

            }
        }

        #[derive( Debug, Clone )]
        pub struct FunctionDataImpl {
            function: wit_parser::Function,
            return_kind: wasm_link::ReturnKind,
        }

        impl wasm_link::FunctionData for FunctionDataImpl {
            fn name( &self ) -> &str { &self.function.name }
            fn return_kind( &self ) -> wasm_link::ReturnKind { self.return_kind.clone() }
            fn is_method( &self ) -> bool {
                match self.function.kind {
                    wit_parser::FunctionKind::Freestanding
                    | wit_parser::FunctionKind::Static( _ )
                    | wit_parser::FunctionKind::Constructor( _ ) => false,
                    wit_parser::FunctionKind::Method( _ ) => true,
                    wit_parser::FunctionKind::AsyncFreestanding
                    | wit_parser::FunctionKind::AsyncMethod( _ )
                    | wit_parser::FunctionKind::AsyncStatic( _ )
                    => unimplemented!( "Async functions are not yet implemented" ),
                }
            }
        }

        impl wasm_link::InterfaceData for InterfaceDir {

            type Id = String ;
            type Error = FixtureError ;
            type Function = FunctionDataImpl ;
            type FunctionIter<'a> = Vec<&'a FunctionDataImpl>;
            type ResourceIter<'a> = &'a [String];

            fn id( &self ) -> Result<&Self::Id, Self::Error> { Ok( &self.id ) }
            fn package_name( &self ) -> Result<&str, Self::Error> { Ok( &self.wit_data.package ) }
            fn cardinality( &self ) -> Result<&wasm_link::InterfaceCardinality, Self::Error> { Ok( &self.cardinality ) }
            fn functions<'a>( &'a self ) -> Result<Self::FunctionIter<'a>, Self::Error> { Ok( self.wit_data.functions.values().collect()) }
            fn resources<'a>( &'a self ) -> Result<Self::ResourceIter<'a>, Self::Error> { Ok( &self.wit_data.resources ) }

        }

        #[derive( Debug, Clone )]
        pub struct PluginDir {
            id: String,
            plug: String,
            sockets: Vec<String>,
            wasm_path: std::path::PathBuf,
        }

        impl PluginDir {

            #[allow( unused )]
            pub fn new( id: &'static str ) -> Result<Self, FixtureError> {

                let root_path = std::path::PathBuf::from( FIXTURES_DIR ).join( "plugins" ).join( id );
                let manifest_path = root_path.join( "manifest.toml" );
                let manifest_data: PluginManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;

                let wasm_path = root_path.join( "root.wasm" );
                let wasm_path = if wasm_path.exists() { wasm_path } else { root_path.join( "root.wat" ) };

                Ok( Self { id: id.to_string(), plug: manifest_data.plug, sockets: manifest_data.sockets, wasm_path })

            }
        }

        impl wasm_link::PluginData for PluginDir {

            type Id = String ;
            type InterfaceId = String ;
            type Error = FixtureError ;
            type SocketIter<'b> = &'b [Self::InterfaceId];

            fn id( &self ) -> Result<&Self::Id, Self::Error> { Ok( &self.id ) }
            fn plug( &self ) -> Result<&Self::Id, Self::Error> {
                Ok( &self.plug )
            }
            fn sockets<'b>( &'b self ) -> Result<Self::SocketIter<'b>, Self::Error> {
                Ok( &self.sockets )
            }

            fn component( &self, engine: &wasm_link::Engine ) -> Result<wasm_link::Component, Self::Error> {
                wasm_link::Component::from_file( engine, &self.wasm_path ).map_err(| e | FixtureError::WasmLoad( e.to_string() ))
            }

        }

        #[derive( Debug, serde::Deserialize )]
        struct PluginManifestData {
            plug: String,
            #[serde( default )]
            sockets: Vec<String>,
        }

        #[derive( Debug, serde::Deserialize )]
        struct InterfaceManifestData {
            cardinality: __InterfaceCardinality,
        }

        #[derive( Debug, serde::Deserialize )]
        enum __InterfaceCardinality {
            AtMostOne,
            ExactlyOne,
            AtLeastOne,
            Any,
        }

        impl From<__InterfaceCardinality> for wasm_link::InterfaceCardinality {
            fn from( parsed: __InterfaceCardinality ) -> Self {
                match parsed {
                    __InterfaceCardinality::AtMostOne => wasm_link::InterfaceCardinality::AtMostOne,
                    __InterfaceCardinality::ExactlyOne => Self::ExactlyOne,
                    __InterfaceCardinality::AtLeastOne => Self::AtLeastOne,
                    __InterfaceCardinality::Any => Self::Any,
                }
            }
        }

        #[derive( Debug, Clone )]
        struct InterfaceWitData {
            package: String,
            functions: std::collections::HashMap<String, FunctionDataImpl>,
            resources: Vec<String>,
        }

        fn parse_wit( root_path: &std::path::Path ) -> Result<InterfaceWitData, FixtureError> {

            let mut resolve = wit_parser::Resolve::new();
            let _ = resolve.push_path( root_path )?;

            let interface = resolve.interfaces.iter().find(|( _, interface )| match &interface.name {
                Some( name ) => name.as_str() == "root",
                Option::None => false,
            }).ok_or( FixtureError::NoRootInterface )?.1;

            let package = resolve.packages
                .get( interface.package.ok_or( FixtureError::NoPackage )? )
                .ok_or( FixtureError::NoPackage )?
                .name.to_string();

            let functions = interface.functions.iter()
                .map(|( _, function )| Ok(( function.name.clone(), FunctionDataImpl {
                    function: function.clone(),
                    return_kind: parse_return_kind( &resolve, function.result )?,
                })))
                .collect::<Result<_,FixtureError>>()?;

            let resources = interface.types.iter().filter_map(|( name, wit_type_id )| match resolve.types.get( *wit_type_id ) {
                Option::None => Some( Err( FixtureError::UndeclaredType( *wit_type_id ) )),
                Some( wit_type ) if wit_type.kind == wit_parser::TypeDefKind::Resource => Some( Ok( name.to_string() )),
                _ => None,
            }).collect::<Result<_, FixtureError>>()?;

            Ok( InterfaceWitData { package, functions, resources })
        }

        fn parse_return_kind(
            resolve: &wit_parser::Resolve,
            result: Option<wit_parser::Type>
        ) -> Result<wasm_link::ReturnKind, FixtureError> {
            let Some( return_type ) = result else { return Ok( wasm_link::ReturnKind::Void )};
            Ok( match has_resource( resolve, return_type )? {
                false => wasm_link::ReturnKind::AssumeNoResources,
                true => wasm_link::ReturnKind::MayContainResources,
            })
        }

        fn has_resource( resolve: &wit_parser::Resolve, wit_type: wit_parser::Type ) -> Result<bool, FixtureError> {
            Ok( match wit_type {
                wit_parser::Type::Id( id ) => match &resolve.types.get( id )
                    .ok_or_else(|| FixtureError::UndeclaredType( id ))?
                    .kind
                {
                    wit_parser::TypeDefKind::Resource
                    | wit_parser::TypeDefKind::Handle( wit_parser::Handle::Own( _ )) => true,

                    wit_parser::TypeDefKind::Handle( wit_parser::Handle::Borrow( _ ))
                    | wit_parser::TypeDefKind::Flags( _ )
                    | wit_parser::TypeDefKind::Enum( _ )
                    | wit_parser::TypeDefKind::Future( Option::None )
                    | wit_parser::TypeDefKind::Stream( Option::None )
                    | wit_parser::TypeDefKind::Unknown => false,

                    wit_parser::TypeDefKind::Option( wit_type )
                    | wit_parser::TypeDefKind::List( wit_type )
                    | wit_parser::TypeDefKind::FixedSizeList( wit_type, _ )
                    | wit_parser::TypeDefKind::Future( Some( wit_type ))
                    | wit_parser::TypeDefKind::Stream( Some( wit_type ))
                    | wit_parser::TypeDefKind::Type( wit_type ) => has_resource( resolve, *wit_type )?,

                    wit_parser::TypeDefKind::Map( key_type, value_type ) =>
                        has_resource( resolve, *key_type )?
                        || has_resource( resolve, *value_type )?,

                    wit_parser::TypeDefKind::Result( result ) =>
                        ( match result.ok { Some( wit_type ) => has_resource( resolve, wit_type )?, _ => false, })
                        || match result.err { Some( wit_type ) => has_resource( resolve, wit_type )?, _ => false, },

                    wit_parser::TypeDefKind::Record( record ) => record.fields.iter().try_fold( false, | acc, field |
                        Result::<_, FixtureError>::Ok( acc || has_resource( resolve, field.ty )? )
                    )?,

                    wit_parser::TypeDefKind::Tuple( tuple ) => tuple.types.iter().try_fold( false, | acc, &item |
                        Result::<_, FixtureError>::Ok( acc || has_resource( resolve, item )? )
                    )?,

                    wit_parser::TypeDefKind::Variant( variant ) => variant.cases.iter().try_fold( false, | acc, case |
                        Result::<_, FixtureError>::Ok( acc || match case.ty {
                            Some( wit_type ) => has_resource( resolve, wit_type )?,
                            Option::None => false,
                        })
                    )?,
                },
                _ => false,
            })

        }
    }};
}
