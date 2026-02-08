#[macro_export]
macro_rules! fixtures {

    {
        const ROOT  = $root:literal ;
        interfaces  = [ $($interface:literal),* $(,)? ];
        plugins     = [ $($plugin:literal),* $(,)? ];
    } => ( mod fixtures {

        #[allow( dead_code )]
        pub const ROOT: &'static str = $root ;

        pub fn interface( name: &str ) -> $crate::fixture_linking::InterfaceData {
            $crate::fixture_linking::parse_interface( $crate::fixture_linking::strip_rs( file!() ), name )
                .expect( &format!( "Interface {} failed to initialise", name ))
        }

        pub fn plugin( name: &str, engine: &wasm_link::Engine ) -> $crate::fixture_linking::PluginData {
            $crate::fixture_linking::parse_plugin( $crate::fixture_linking::strip_rs( file!() ), name, engine )
                .expect( &format!( "Plugin {} failed to initialise", name ))
        }

        #[allow( dead_code )]
        pub fn interfaces() -> Vec<$crate::fixture_linking::InterfaceData> {
            vec![$( interface( $interface ) ),*]
        }

        #[allow( dead_code )]
        pub fn plugins( engine: &wasm_link::Engine ) -> Vec<$crate::fixture_linking::PluginData> {
            vec![$( plugin( $plugin, engine ) ),*]
        }

    });
}

mod fixture_linking {

    use std::collections::{ HashMap, HashSet };
    use wasm_link::{ Component, Engine, Interface, Function, Plugin, Socket, NEMap };

    pub const fn strip_rs( path: &'static str ) -> &'static str {
        match path.as_bytes() {
            [rest @ .., b'.', b'r', b's'] => {
                // SAFETY: we just checked that the last three bytes are ".rs",
                // so the split is at a UTF-8 boundary.
                unsafe { core::str::from_utf8_unchecked( rest ) }
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

    /// Test context that implements `PluginContext`.
    pub struct TestContext {
        pub resource_table: wasm_link::ResourceTable,
    }

    impl wasm_link::PluginContext for TestContext {
        fn resource_table( &mut self ) -> &mut wasm_link::ResourceTable {
            &mut self.resource_table
        }
    }

    /// Parsed interface data from fixtures.
    #[allow( dead_code )]
    pub struct InterfaceData {
        /// The WIT package name (e.g., "test:primitive")
        pub package: String,
        /// The interface name (e.g., "root")
        pub name: String,
        /// The parsed interface with functions and resources
        pub interface: Interface,
        /// Cardinality hint from manifest (for constructing the right Socket variant)
        pub cardinality: Cardinality,
    }

    /// Parsed plugin data from fixtures.
    #[allow( dead_code )]
    pub struct PluginData {
        /// Plugin ID
        pub id: String,
        /// The Plugin ready to link
        pub plugin: Plugin<TestContext>,
        /// Which interface this plugin implements
        pub plug: String,
        /// Which interfaces this plugin depends on
        pub sockets: Vec<String>,
    }

    #[allow( dead_code )]
    #[derive( Debug, Clone, Copy, serde::Deserialize )]
    pub enum Cardinality {
        AtMostOne,
        ExactlyOne,
        AtLeastOne,
        Any,
    }

    #[allow( dead_code )]
    impl Cardinality {
        /// Creates a Socket with a single value based on this cardinality.
        pub fn socket_one<T, Id: std::hash::Hash + Eq>( self, id: Id, value: T ) -> Socket<T, Id> {
            match self {
                Self::AtMostOne => Socket::AtMostOne( Some(( id, value ))),
                Self::ExactlyOne => Socket::ExactlyOne( id, value ),
                Self::AtLeastOne | Self::Any => panic!( "socket_one requires AtMostOne or ExactlyOne cardinality" ),
            }
        }

        /// Creates a Socket with multiple values based on this cardinality.
        pub fn socket_many<T, Id: std::hash::Hash + Eq + Clone>( self, values: HashMap<Id, T> ) -> Socket<T, Id> {
            match self {
                Self::AtLeastOne => Socket::AtLeastOne(
                    NEMap::try_from( values ).expect( "AtLeastOne requires at least one value" )
                ),
                Self::Any => Socket::Any( values ),
                Self::AtMostOne | Self::ExactlyOne => panic!( "socket_many requires AtLeastOne or Any cardinality" ),
            }
        }

        /// Creates an empty Socket for optional cardinalities.
        pub fn socket_empty<T, Id: std::hash::Hash + Eq>( self ) -> Socket<T, Id> {
            match self {
                Self::AtMostOne => Socket::AtMostOne( None ),
                Self::Any => Socket::Any( HashMap::new() ),
                Self::ExactlyOne | Self::AtLeastOne => panic!( "socket_empty requires AtMostOne or Any cardinality" ),
            }
        }
    }

    pub fn parse_interface( fixtures_dir: &'static str, id: &str ) -> Result<InterfaceData, FixtureError> {

        let root_path = std::path::PathBuf::from( fixtures_dir ).join( "interfaces" ).join( id );
        let manifest_path = root_path.join( "manifest.toml" );
        let manifest_data: InterfaceManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;

        let wit_data = parse_wit( &root_path )?;

        Ok( InterfaceData {
            package: wit_data.package,
            name: wit_data.interface_name,
            interface: Interface::new( wit_data.functions, wit_data.resources ),
            cardinality: manifest_data.cardinality,
        })

    }

    pub fn parse_plugin(
        fixtures_dir: &'static str,
        id: &str,
        engine: &Engine,
    ) -> Result<PluginData, FixtureError> {

        let root_path = std::path::PathBuf::from( fixtures_dir ).join( "plugins" ).join( id );
        let manifest_path = root_path.join( "manifest.toml" );
        let manifest_data: PluginManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;

        let wasm_path = root_path.join( "root.wasm" );
        let wasm_path = if wasm_path.exists() { wasm_path } else { root_path.join( "root.wat" ) };

        let component = Component::from_file( engine, &wasm_path )
            .map_err(| e | FixtureError::WasmLoad( e.to_string() ))?;

        Ok( PluginData {
            id: id.to_string(),
            plugin: Plugin::new(
                component,
                TestContext { resource_table: wasm_link::ResourceTable::new() },
            ),
            plug: manifest_data.plug,
            sockets: manifest_data.sockets,
        })

    }

    #[derive( Debug, serde::Deserialize )]
    struct PluginManifestData {
        plug: String,
        #[serde( default )]
        sockets: Vec<String>,
    }

    #[derive( Debug, serde::Deserialize )]
    struct InterfaceManifestData {
        cardinality: Cardinality,
    }

    struct InterfaceWitData {
        package: String,
        interface_name: String,
        functions: HashMap<String, Function>,
        resources: HashSet<String>,
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
            .map(|( _, function )| Ok(( function.name.clone(), Function::new(
                parse_return_kind( &resolve, function.result )?,
                match function.kind {
                    wit_parser::FunctionKind::Freestanding
                    | wit_parser::FunctionKind::AsyncFreestanding
                    | wit_parser::FunctionKind::Static( _ )
                    | wit_parser::FunctionKind::AsyncStatic( _ )
                    | wit_parser::FunctionKind::Constructor( _ ) => false,
                    wit_parser::FunctionKind::Method( _ )
                    | wit_parser::FunctionKind::AsyncMethod( _ ) => true,
                },
            ))))
            .collect::<Result<HashMap<_, _>,FixtureError>>()?;

        let resources = interface.types.iter().filter_map(|( name, wit_type_id )| match resolve.types.get( *wit_type_id ) {
            Option::None => Some( Err( FixtureError::UndeclaredType( *wit_type_id ) )),
            Some( wit_type ) if wit_type.kind == wit_parser::TypeDefKind::Resource => Some( Ok( name.clone() )),
            _ => None,
        }).collect::<Result<_, FixtureError>>()?;

        let interface_name = interface.name.clone().ok_or( FixtureError::NoRootInterface )?;

        Ok( InterfaceWitData { package, interface_name, functions, resources })
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
}
