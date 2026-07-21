use std::collections::{ HashMap, VecDeque };
use std::sync::{ Arc, Mutex };

use futures::task::{ FutureObj, Spawn };
use wasm_link::{ Binding, DispatchError, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root", sync_root: "sync-root" };
	plugins  = { plugin: "plugin", sync_plugin: "sync-plugin" };
}

#[derive( Clone, Default )]
struct ManualExecutor( Arc<Mutex<VecDeque<FutureObj<'static, ()>>>> );

impl ManualExecutor {
	fn run( &self ) {
		while let Some( future ) = self.0.lock().expect( "manual executor lock poisoned" ).pop_front() {
			std::thread::spawn( move || futures::executor::block_on( future ))
				.join().expect( "manual executor task panicked" );
		}
	}

	fn cancel( &self ) {
		drop( self.0.lock().expect( "manual executor lock poisoned" ).pop_front() );
	}
}

impl Spawn for ManualExecutor {
	fn spawn_obj( &self, future: FutureObj<'static, ()> ) -> Result<(), futures::task::SpawnError> {
		self.0.lock().expect( "manual executor lock poisoned" ).push_back( future );
		Ok(())
	}
}

#[test]
fn services_callers_round_robin_and_recovers_canceled_capacity() {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let executor = ManualExecutor::default();
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let instance = plugins.plugin.plugin
			.instantiate_async( &engine, &linker, executor.clone() ).await
			.expect( "failed to instantiate scheduler fixture" );
		let binding_a = Binding::new(
			bindings.root.package.clone(),
			HashMap::from([( bindings.root.name.clone(), bindings.root.spec.clone() )]),
			ExactlyOne( "plugin".to_string(), instance.clone() ),
		);
		let binding_b = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "plugin".to_string(), instance ),
		);

		let mut a1 = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 1 ) ]));
		let mut a2 = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 2 ) ]));
		let mut a3 = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 3 ) ]));
		let mut b1 = Box::pin( binding_b.dispatch( "root", "run", &[ Val::U32( 10 ) ]));
		assert!( futures::poll!( a1.as_mut() ).is_pending() );
		assert!( futures::poll!( a2.as_mut() ).is_pending() );
		assert!( futures::poll!( a3.as_mut() ).is_pending() );
		assert!( futures::poll!( b1.as_mut() ).is_pending() );
		executor.run();

		assert_result( a1.await, 1, 1 );
		assert_result( b1.await, 2, 10 );
		assert_result( a2.await, 3, 2 );
		assert_result( a3.await, 4, 3 );

		let flood_args = [ Val::U32( 20 ) ];
		let mut flood = ( 0..1_024 ).map(| _ |
			Box::pin( binding_a.dispatch( "root", "run", &flood_args ))
		).collect::<Vec<_>>();
		for call in &mut flood {
			assert!( futures::poll!( call.as_mut() ).is_pending() );
		}
		let mut newcomer = Box::pin( binding_b.dispatch( "root", "run", &[ Val::U32( 21 ) ]));
		assert!( futures::poll!( newcomer.as_mut() ).is_pending() );
		let mut rejected = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 0 ) ]));
		match futures::poll!( rejected.as_mut() ) {
			std::task::Poll::Ready( Ok( ExactlyOne( _, Err( DispatchError::DispatchQueueFull )))) => {}
			value => panic!( "expected caller count rejection, found {value:#?}" ),
		}
		executor.run();
		assert_result( newcomer.await, 6, 21 );
		for ( index, call ) in flood.iter_mut().enumerate() {
			let sequence = match index { 0 => 5, _ => index as u32 + 6 };
			assert_result( call.as_mut().await, sequence, 20 );
		}

		for _ in 0..1_024 {
			let mut canceled = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 0 ) ]));
			assert!( futures::poll!( canceled.as_mut() ).is_pending() );
		}
		let mut rejected = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 0 ) ]));
		match futures::poll!( rejected.as_mut() ) {
			std::task::Poll::Ready( Ok( ExactlyOne( _, Err( DispatchError::DispatchQueueFull )))) => {}
			value => panic!( "expected caller count rejection, found {value:#?}" ),
		}
		executor.run();

		let mut recovered = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 11 ) ]));
		assert!( futures::poll!( recovered.as_mut() ).is_pending() );
		executor.run();
		assert_result( recovered.await, 1_030, 11 );

		let mut canceled_task = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 12 ) ]));
		assert!( futures::poll!( canceled_task.as_mut() ).is_pending() );
		executor.cancel();
		match canceled_task.await {
			Ok( ExactlyOne( _, Err( DispatchError::ExecutorUnavailable ))) => {}
			value => panic!( "expected canceled drain task to report executor failure, found {value:#?}" ),
		}
		let mut after_executor_cancel = Box::pin( binding_a.dispatch( "root", "run", &[ Val::U32( 13 ) ]));
		assert!( futures::poll!( after_executor_cancel.as_mut() ).is_pending() );
		executor.run();
		assert_result( after_executor_cancel.await, 1_031, 13 );
	});
}

fn assert_result(
	result: Result<ExactlyOne<String, Result<Val, DispatchError>>, DispatchError>,
	sequence: u32,
	value: u32,
) {
	assert!( matches!(
		result,
		Ok( ExactlyOne( _, Ok( Val::Tuple( values ))))
			if values == vec![ Val::U32( sequence ), Val::U32( value ) ]
	));
}

#[test]
fn synchronous_callers_wait_for_a_shared_destination() {
	let engine = Engine::default();
	let linker = Linker::new( &engine );
	let plugins = fixtures::plugins( &engine );
	let bindings = fixtures::bindings();
	let instance = plugins.sync_plugin.plugin
		.instantiate( &engine, &linker )
		.expect( "failed to instantiate sync scheduler fixture" );
	let binding_a = Arc::new( Binding::new(
		bindings.sync_root.package.clone(),
		HashMap::from([( bindings.sync_root.name.clone(), bindings.sync_root.spec.clone() )]),
		ExactlyOne( "plugin".to_string(), instance.clone() ),
	));
	let binding_b = Arc::new( Binding::new(
		bindings.sync_root.package,
		HashMap::from([( bindings.sync_root.name, bindings.sync_root.spec )]),
		ExactlyOne( "plugin".to_string(), instance ),
	));
	let start = Arc::new( std::sync::Barrier::new( 17 ));
	let threads = ( 0..16 ).map(| value | {
		let binding = match value % 2 { 0 => Arc::clone( &binding_a ), _ => Arc::clone( &binding_b )};
		let start = Arc::clone( &start );
		std::thread::spawn( move || {
			start.wait();
			binding.dispatch( "root", "run", &[ Val::U32( value ) ])
		})
	}).collect::<Vec<_>>();
	start.wait();

	let mut sequences = threads.into_iter().map(| thread | {
		let result = thread.join().expect( "sync dispatch thread panicked" );
		match result {
			Ok( ExactlyOne( _, Ok( Val::Tuple( values )))) => match values.as_slice() {
				[ Val::U32( sequence ), Val::U32( _ ) ] => *sequence,
				_ => panic!( "unexpected sync response values: {values:#?}" ),
			},
			value => panic!( "shared sync destination rejected a caller: {value:#?}" ),
		}
	}).collect::<Vec<_>>();
	sequences.sort_unstable();
	assert_eq!( sequences, ( 1..=16 ).collect::<Vec<_>>() );
}
