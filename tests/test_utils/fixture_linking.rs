#[macro_export]
macro_rules! fixtures {

    {
        const ROOT  = $ROOT:literal ;
        interfaces  = [];
        plugins     = [];
    } => ( mod fixtures {

        pub const ROOT: &'static str = $root ;
        fixtures!( @interfaces );
        fixtures!( @plugins );

    });

    {
        const ROOT  = $root:literal ;
        interfaces  = [ $($interface:literal),* $(,)? ];
        plugins     = [];
    } => ( mod fixtures {

        pub const ROOT: &'static str = $root ;
        fixtures!( @interfaces $($interface),* );
        fixtures!( @plugins );

    });

    {
        const ROOT  = $root:literal ;
        interfaces  = [ $($interface:literal),* $(,)? ];
        plugins     = [ $($plugin:literal),* $(,)? ];
    } => ( mod fixtures {

        pub const ROOT: &'static str = $root ;
        fixtures!( @interfaces $($interface),* );
        fixtures!( @plugins $($plugin),* );

    });

    ( @interfaces ) => {
        pub fn interfaces() -> Vec<wasm_link::Binding<String>> { Vec::with_capacity( 0 ) }
    };
    ( @interfaces $($interface:literal),* $(,)? ) => (
        pub fn interfaces() -> Vec<wasm_link::Binding<String>> { vec![ $(
            $crate::fixture_linking::parse_binding( $crate::fixture_linking::strip_rs( file!() ), $interface )
                .expect( format!( "Interface {} failed to initialise", $interface ).as_str())
        ),* ]}
    );

    ( @plugins ) => {
        pub fn plugins( _: &wasm_link::Engine ) -> Vec<wasm_link::Plugin<String, String, $crate::fixture_linking::TestContext>> {
            Vec::with_capacity( 0 )
        }
    };
    ( @plugins $($plugin:literal),* $(,)? ) => (
        pub fn plugins( engine: &wasm_link::Engine ) -> Vec<wasm_link::Plugin<String, String, $crate::fixture_linking::TestContext>> { vec![ $(
            $crate::fixture_linking::parse_plugin( $crate::fixture_linking::strip_rs( file!() ), $plugin, engine )
                .expect( format!( "Plugin {} failed to initialise", $plugin ).as_str())
        ),* ]}
    );
}

mod fixture_linking {

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

    pub fn parse_binding( fixtures_dir: &'static str, id: &str ) -> Result<wasm_link::Binding<String>, FixtureError> {

        let root_path = std::path::PathBuf::from( fixtures_dir ).join( "interfaces" ).join( id );
        let manifest_path = root_path.join( "manifest.toml" );
        let manifest_data: InterfaceManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;
        let cardinality = manifest_data.cardinality.into();

        let wit_data = parse_wit( &root_path )?;

        Ok( wasm_link::Binding::new(
            id.to_string(),
            cardinality,
            wit_data.package,
            vec![ wasm_link::Interface::new( "root", wit_data.functions, wit_data.resources ) ],
        ))

    }

    pub fn parse_plugin(
        fixtures_dir: &'static str,
        id: &str,
        engine: &wasm_link::Engine,
    ) -> Result<wasm_link::Plugin<String, String, TestContext>, FixtureError> {

        let root_path = std::path::PathBuf::from( fixtures_dir ).join( "plugins" ).join( id );
        let manifest_path = root_path.join( "manifest.toml" );
        let manifest_data: PluginManifestData = toml::from_str( &std::fs::read_to_string( manifest_path )?)?;

        let wasm_path = root_path.join( "root.wasm" );
        let wasm_path = if wasm_path.exists() { wasm_path } else { root_path.join( "root.wat" ) };

        let component = wasm_link::Component::from_file( engine, &wasm_path )
            .map_err(| e | FixtureError::WasmLoad( e.to_string() ))?;

        Ok( wasm_link::Plugin::new(
            id.to_string(),
            manifest_data.plug,
            manifest_data.sockets,
            component,
            TestContext { resource_table: wasm_link::ResourceTable::new() },
        ))

    }

    #[derive( Debug, serde::Deserialize )]
    struct PluginManifestData {
        plug: String,
        #[serde( default )]
        sockets: Vec<String>,
    }

    #[derive( Debug, serde::Deserialize )]
    struct InterfaceManifestData {
        cardinality: __Cardinality,
    }

    #[derive( Debug, serde::Deserialize )]
    enum __Cardinality {
        AtMostOne,
        ExactlyOne,
        AtLeastOne,
        Any,
    }

    impl From<__Cardinality> for wasm_link::Cardinality {
        fn from( parsed: __Cardinality ) -> Self {
            match parsed {
                __Cardinality::AtMostOne => wasm_link::Cardinality::AtMostOne,
                __Cardinality::ExactlyOne => Self::ExactlyOne,
                __Cardinality::AtLeastOne => Self::AtLeastOne,
                __Cardinality::Any => Self::Any,
            }
        }
    }

    struct InterfaceWitData {
        package: String,
        functions: Vec<wasm_link::Function>,
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
            .map(|( _, function )| Ok( wasm_link::Function::new(
                function.name.clone(),
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
            )))
            .collect::<Result<_,FixtureError>>()?;

        let resources = interface.types.iter().filter_map(|( name, wit_type_id )| match resolve.types.get( *wit_type_id ) {
            Option::None => Some( Err( FixtureError::UndeclaredType( *wit_type_id ) )),
            Some( wit_type ) if wit_type.kind == wit_parser::TypeDefKind::Resource => Some( Ok( name.clone() )),
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
}
