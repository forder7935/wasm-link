use wasmtime::{ AsContextMut, Config, Engine, Store };
use wasmtime::component::{ Component, FutureReader, Linker, ResourceTable, StreamReader, Val };

use super::wrap_resources ;
use crate::PluginContext ;



struct Context { table: ResourceTable }

impl PluginContext for Context {
	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.table }
}

#[test]
fn wraps_nested_component_values_without_resources() -> Result<(), crate::DispatchError> {
	let mut store = Store::new( &Engine::default(), Context { table: ResourceTable::new() });
	let values = [
		Val::Bool( true ),
		Val::List( vec![ Val::U32( 1 ) ]),
		Val::Map( vec![( Val::String( "key".to_string() ), Val::U32( 2 ))]),
		Val::Record( vec![( "field".to_string(), Val::U32( 3 ))]),
		Val::Tuple( vec![ Val::U32( 4 ) ]),
		Val::Variant( "value".to_string(), Some( Box::new( Val::U32( 5 )))),
		Val::Option( Some( Box::new( Val::U32( 6 )))),
		Val::Result( Ok( Some( Box::new( Val::U32( 7 ))))),
		Val::Result( Err( Some( Box::new( Val::U32( 8 ))))),
	];

	values.into_iter().try_for_each(| value |
		wrap_resources( value, "plugin".to_string(), &mut store.as_context_mut() ).map( drop )
	)?;
	Ok(())
}

#[test]
fn rejects_async_values_during_cross_plugin_transfer() -> Result<(), Box<dyn std::error::Error>> {
	let mut config = Config::new();
	config.concurrency_support( true );
	let engine = Engine::new( &config )?;
	let mut store = Store::new( &engine, Context { table: ResourceTable::new() });
	let future = FutureReader::new( &mut store, async { Ok::<_, wasmtime::Error>( 1_u32 )})?
		.try_into_future_any( &mut store )?;
	let stream = StreamReader::new( &mut store, vec![ 1_u32 ])?
		.try_into_stream_any( &mut store )?;

	assert!( matches!(
		wrap_resources( Val::Future( future ), "plugin".to_string(), &mut store.as_context_mut() ),
		Err( crate::DispatchError::UnsupportedType( name )) if name == "future"
	));
	assert!( matches!(
		wrap_resources( Val::Stream( stream ), "plugin".to_string(), &mut store.as_context_mut() ),
		Err( crate::DispatchError::UnsupportedType( name )) if name == "stream"
	));
	Ok(())
}

#[test]
fn rejects_error_contexts_during_cross_plugin_transfer() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let mut config = Config::new();
		config.wasm_component_model_async( true );
		config.wasm_component_model_error_context( true );
		let engine = Engine::new( &config )?;
		let component = Component::from_file(
			&engine,
			concat!( env!( "CARGO_MANIFEST_DIR" ), "/tests/linker/error_context.wat" ),
		)?;
		let linker = Linker::<Context>::new( &engine );
		let mut store = Store::new( &engine, Context { table: ResourceTable::new() });
		let instance = linker.instantiate_async( &mut store, &component ).await?;
		let function = instance.get_func( &mut store, "make-error-context" )
			.ok_or( "missing make-error-context export" )?;
		let mut results = [ Val::Bool( false ) ];
		function.call_async( &mut store, &[], &mut results ).await?;
		assert!( matches!(
			wrap_resources( results[0].clone(), "plugin".to_string(), &mut store.as_context_mut() ),
			Err( crate::DispatchError::UnsupportedType( name )) if name == "error-context"
		));
		Ok::<_, Box<dyn std::error::Error>>(())
	})
}
