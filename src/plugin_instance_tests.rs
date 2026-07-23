use wasmtime::{ Config, Engine, Store };
use wasmtime::component::{ Component, FutureReader, Linker, ResourceTable, StreamReader, Val };

use super::{ PluginInstanceAsync, PluginInstanceSync, ensure_supported_value };
use crate::{ DispatchError, PluginContext };

struct Context { table: ResourceTable }

impl PluginContext for Context {
	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.table }
}

#[test]
fn plugin_instances_are_send_and_sync() {
	fn assert_send_sync<T: Send + Sync>() {}
	assert_send_sync::<PluginInstanceSync<Context>>();
	assert_send_sync::<PluginInstanceAsync<Context>>();
}

#[test]
fn accepts_nested_component_values() -> Result<(), DispatchError> {
	let value = Val::Record( vec![
		( "list".to_string(), Val::List( vec![ Val::U32( 1 ) ])),
		( "tuple".to_string(), Val::Tuple( vec![ Val::U32( 2 ) ])),
		( "map".to_string(), Val::Map( vec![( Val::String( "key".to_string() ), Val::U32( 3 ))])),
		( "variant".to_string(), Val::Variant( "some".to_string(), Some( Box::new( Val::U32( 4 ))))),
		( "option".to_string(), Val::Option( Some( Box::new( Val::U32( 5 ))))),
		( "ok".to_string(), Val::Result( Ok( Some( Box::new( Val::U32( 6 )))))),
		( "err".to_string(), Val::Result( Err( Some( Box::new( Val::U32( 7 )))))),
	]);
	ensure_supported_value( &value )
}

#[test]
fn rejects_future_and_stream_values() -> Result<(), Box<dyn std::error::Error>> {
	let mut config = Config::new();
	config.concurrency_support( true );
	let engine = Engine::new( &config )?;
	let mut store = Store::new( &engine, Context { table: ResourceTable::new() });
	let future = FutureReader::new( &mut store, async { Ok::<_, wasmtime::Error>( 1_u32 )})?
		.try_into_future_any( &mut store )?;
	let stream = StreamReader::new( &mut store, vec![ 1_u32 ])?
		.try_into_stream_any( &mut store )?;

	assert!( matches!(
		ensure_supported_value( &Val::Future( future )),
		Err( DispatchError::UnsupportedType( name )) if name == "future"
	));
	assert!( matches!(
		ensure_supported_value( &Val::Stream( stream )),
		Err( DispatchError::UnsupportedType( name )) if name == "stream"
	));
	Ok(())
}

#[test]
fn rejects_error_context_values() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let mut config = Config::new();
		config.wasm_component_model_async( true );
		config.wasm_component_model_error_context( true );
		let engine = Engine::new( &config )?;
		let component = Component::from_file(
			&engine,
			concat!( env!( "CARGO_MANIFEST_DIR" ), "/tests/plugin_instance/error_context.wat" ),
		)?;
		let linker = Linker::<Context>::new( &engine );
		let mut store = Store::new( &engine, Context { table: ResourceTable::new() });
		let instance = linker.instantiate_async( &mut store, &component ).await?;
		let function = instance.get_func( &mut store, "make-error-context" )
			.ok_or( "missing make-error-context export" )?;
		let mut results = [ Val::Bool( false ) ];
		function.call_async( &mut store, &[], &mut results ).await?;
		assert!( matches!(
			ensure_supported_value( &results[0] ),
			Err( DispatchError::UnsupportedType( name )) if name == "error-context"
		));
		Ok(())
	})
}
