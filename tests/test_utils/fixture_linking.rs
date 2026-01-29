#[macro_export]
macro_rules! bind_fixtures {
    ($( $segment:expr ),+ $(,)?) => { mod fixtures {

        #[derive( Debug, thiserror::Error )]
        pub enum FixtureError {
            #[error("IO error: {0}")] Io( #[from] std::io::Error ),
            #[error("TOML parse error: {0}")] Toml( #[from] toml::de::Error ),
            #[error("WIT parser error: {0}")] WitParser( #[from] anyhow::Error ),
            #[error("No root interface found")] NoRootInterface,
            #[error("No package for root interface")] NoPackage,
            #[error("Undeclared type: {0:?}")] UndeclaredType( wit_parser::TypeId ),
            #[error("WASM load error: {0}")] WasmLoad( String ),
            #[error("Interface not found: {0}")] InterfaceNotFound( String ),
        }

        fn fixture_path() -> std::path::PathBuf {
            std::path::PathBuf::from( env!( "CARGO_MANIFEST_DIR" ))
                .join( "tests" )
                $(.join( $segment ))+
        }

        fn interface_name_to_id( name: &str ) -> Option<wasm_compose::InterfaceId> {
            // Use the generated dir_name function in reverse
            let mut id = 0u64;
            loop {
                let candidate = wasm_compose::InterfaceId::new( id );
                match interfaces::dir_name( candidate ) {
                    Some( dir_name ) if dir_name == name => return Some( candidate ),
                    Some( _ ) => id += 1,
                    None => return None,
                }
            }
        }


        #[derive( Debug )]
        pub struct InterfaceDir {
            id: wasm_compose::InterfaceId,
            cardinality: wasm_compose::InterfaceCardinality,
            wit_data: InterfaceWitData,
        }

        impl InterfaceDir {

            pub fn new( id: wasm_compose::InterfaceId ) -> Result<Self, FixtureError> {

                let dir_name = interfaces::dir_name( id ).ok_or_else(|| FixtureError::Io(
                    std::io::Error::new( std::io::ErrorKind::NotFound, format!( "Interface {} not found", id ))
                ))?;
                let root_path = fixture_path().join( "interfaces" ).join( dir_name );
                let manifest_path = root_path.join( "manifest.toml" );
                let manifest_data: InterfaceManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;
                let cardinality = manifest_data.cardinality.into();

                let wit_data = parse_wit( &root_path )?;

                Ok( Self { id, cardinality, wit_data })

            }
        }

        impl wasm_compose::InterfaceData for InterfaceDir {

            type Error = FixtureError ;
            type FunctionIter<'a> = Vec<&'a wasm_compose::FunctionData>;
            type ResourceIter<'a> = &'a [String];

            fn id( &self ) -> wasm_compose::InterfaceId { self.id }

            fn get_package_name( &self ) -> Result<&str, Self::Error> { Ok( &self.wit_data.package ) }
            fn get_cardinality( &self ) -> Result<&wasm_compose::InterfaceCardinality, Self::Error> { Ok( &self.cardinality ) }
            fn get_functions<'a>( &'a self ) -> Result<Self::FunctionIter<'a>, Self::Error> { Ok( self.wit_data.functions.values().collect()) }
            fn get_resources<'a>( &'a self ) -> Result<Self::ResourceIter<'a>, Self::Error> { Ok( &self.wit_data.resources ) }

        }

        #[derive( Debug )]
        pub struct PluginDir {
            id: wasm_compose::PluginId,
            plug: wasm_compose::InterfaceId,
            sockets: Vec<wasm_compose::InterfaceId>,
            wasm_path: std::path::PathBuf,
        }

        impl PluginDir {

            #[allow( unused )]
            pub fn new( id: wasm_compose::PluginId ) -> Result<Self, FixtureError> {

                let dir_name = plugins::dir_name( id ).ok_or_else(|| FixtureError::Io(
                    std::io::Error::new( std::io::ErrorKind::NotFound, format!( "Plugin {} not found", id ))
                ))?;
                let root_path = fixture_path().join( "plugins" ).join( dir_name );
                let manifest_path = root_path.join( "manifest.toml" );
                let manifest_data: PluginManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;

                let plug = interface_name_to_id( &manifest_data.plug )
                    .ok_or_else(|| FixtureError::InterfaceNotFound( manifest_data.plug.clone() ))?;
                let sockets = manifest_data.sockets.iter()
                    .map(| name | interface_name_to_id( name ).ok_or_else(|| FixtureError::InterfaceNotFound( name.clone() )))
                    .collect::<Result<Vec<_>, _>>()?;

                let wasm_path = root_path.join( "root.wasm" );
                let wasm_path = if wasm_path.exists() { wasm_path } else { root_path.join( "root.wat" ) };

                Ok( Self { id, plug, sockets, wasm_path })

            }
        }

        impl wasm_compose::PluginData for PluginDir {

            type Error = FixtureError ;
            type SocketIter<'a> = &'a [wasm_compose::InterfaceId];

            fn get_id( &self ) -> Result<&wasm_compose::PluginId, Self::Error> { Ok( &self.id ) }
            fn get_plug( &self ) -> Result<&wasm_compose::InterfaceId, Self::Error> {
                Ok( &self.plug )
            }
            fn get_sockets<'a>( &'a self ) -> Result<Self::SocketIter<'a>, Self::Error> {
                Ok( &self.sockets )
            }

            fn component( &self, engine: &wasm_compose::Engine ) -> Result<wasm_compose::Component, Self::Error> {
                wasm_compose::Component::from_file( engine, &self.wasm_path ).map_err(| e | FixtureError::WasmLoad( e.to_string() ))
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

        impl From<__InterfaceCardinality> for wasm_compose::InterfaceCardinality {
            fn from( parsed: __InterfaceCardinality ) -> Self {
                match parsed {
                    __InterfaceCardinality::AtMostOne => wasm_compose::InterfaceCardinality::AtMostOne,
                    __InterfaceCardinality::ExactlyOne => Self::ExactlyOne,
                    __InterfaceCardinality::AtLeastOne => Self::AtLeastOne,
                    __InterfaceCardinality::Any => Self::Any,
                }
            }
        }

        #[derive( Debug )]
        struct InterfaceWitData {
            package: String,
            functions: std::collections::HashMap<String, wasm_compose::FunctionData>,
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
                .map(|( _, function )| Ok(( function.name.clone(), wasm_compose::FunctionData::new(
                    function.clone(),
                    parse_return_type( &resolve, function.result )?,
                ))))
                .collect::<Result<_,FixtureError>>()?;

            let resources = interface.types.iter().filter_map(|( name, wit_type_id )| match resolve.types.get( *wit_type_id ) {
                Option::None => Some( Err( FixtureError::UndeclaredType( *wit_type_id ) )),
                Some( wit_type ) if wit_type.kind == wit_parser::TypeDefKind::Resource => Some( Ok( name.to_string() )),
                _ => None,
            }).collect::<Result<_, FixtureError>>()?;

            Ok( InterfaceWitData { package, functions, resources })
        }

        fn parse_return_type(
            resolve: &wit_parser::Resolve,
            result: Option<wit_parser::Type>
        ) -> Result<wasm_compose::FunctionReturnType, FixtureError> {
            let Some( return_type ) = result else { return Ok( wasm_compose::FunctionReturnType::None )};
            Ok( match has_resource( resolve, return_type )? {
                false => wasm_compose::FunctionReturnType::DataNoResource,
                true => wasm_compose::FunctionReturnType::DataWithResources,
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

        test_macros::generate_fixture_ids!( $( $segment ),+ );
    }};
}
