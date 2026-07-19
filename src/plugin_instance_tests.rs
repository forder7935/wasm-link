use std::sync::atomic::{ AtomicBool, Ordering };

use futures::lock::Mutex;
use wasmtime::{ Config, Engine, Store };
use wasmtime::component::{ Component, FutureReader, Linker, ResourceTable, StreamReader, Val };

use super::{
	Budget, DispatchQueue, MAX_CALLER_BYTES, MAX_CALLER_CALLS,
	MAX_DESTINATION_BYTES, MAX_DESTINATION_CALLS, ensure_supported_value,
	clone_after_wait_with, has_capacity, retained_bytes,
};
use crate::{ DispatchError, PluginContext };

struct Context { table: ResourceTable }

impl PluginContext for Context {
	fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.table }
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

#[test]
fn measures_nested_retained_argument_bytes() {
	let values = vec![
		Val::String( "string".to_string() ),
		Val::Enum( "enum".to_string() ),
		Val::List( vec![ Val::U8( 1 ) ]),
		Val::Tuple( vec![ Val::U16( 2 ) ]),
		Val::Map( vec![( Val::String( "key".to_string() ), Val::U32( 3 ))]),
		Val::Record( vec![( "field".to_string(), Val::U64( 4 ))]),
		Val::Variant( "case".to_string(), Some( Box::new( Val::Bool( true )))),
		Val::Option( Some( Box::new( Val::Char( 'x' )))),
		Val::Result( Ok( Some( Box::new( Val::S32( -1 ))))),
		Val::Flags( vec![ "one".to_string(), "two".to_string() ]),
	];
	assert!( retained_bytes( &values ).is_some_and(| bytes | bytes > std::mem::size_of_val( values.as_slice() )));
}

#[test]
fn enforces_caller_and_destination_count_and_byte_limits() {
	assert!( has_capacity( &Budget::default(), &DispatchQueue::default(), MAX_CALLER_BYTES ));
	assert!( !has_capacity(
		&Budget { calls: MAX_CALLER_CALLS, bytes: 0 }, &DispatchQueue::default(), 0,
	));
	assert!( !has_capacity(
		&Budget { calls: 0, bytes: 1 }, &DispatchQueue::default(), MAX_CALLER_BYTES,
	));
	assert!( !has_capacity(
		&Budget::default(), &DispatchQueue { calls: MAX_DESTINATION_CALLS, ..DispatchQueue::default() }, 0,
	));
	assert!( !has_capacity(
		&Budget::default(),
		&DispatchQueue { bytes: MAX_DESTINATION_BYTES, ..DispatchQueue::default() },
		1,
	));
	assert!( !has_capacity( &Budget { calls: 0, bytes: usize::MAX }, &DispatchQueue::default(), 1 ));
	assert!( !has_capacity(
		&Budget::default(), &DispatchQueue { bytes: usize::MAX, ..DispatchQueue::default() }, 1,
	));
}

#[test]
fn clone_waits_for_a_contended_instance_lock() {
	let mutex = Mutex::new( 42 );
	let lock = mutex.try_lock().expect( "test must hold the instance lock" );
	let waited = AtomicBool::new( false );

	std::thread::scope(| scope | {
		let clone = scope.spawn(|| clone_after_wait_with( &mutex, || {
			waited.store( true, Ordering::Release );
			std::thread::yield_now();
		}));
		while !waited.load( Ordering::Acquire ) { std::thread::yield_now(); }
		drop( lock );
		assert_eq!( clone.join().expect( "clone thread must not panic" ), 42 );
	});
}
