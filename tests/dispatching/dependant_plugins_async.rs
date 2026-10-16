use std::collections::{ HashMap, HashSet, VecDeque };
use std::future::Future ;
use std::sync::{ Arc, Mutex as StdMutex, atomic::{ AtomicUsize, Ordering }};
use std::task::{ Context, Poll };
use wasm_link::{ Binding, Engine, Function, FunctionKind, Interface, Linker, PluginInstanceAsync, ReturnKind, Val };
use wasm_link::cardinality::{ Any, ExactlyOne };
use crate::fixture_linking::TestContext ;

fixtures! {
	bindings = { root: "root", dependency: "dependency" };
	plugins  = {
		startup: "startup",
        cancellation: "cancellation",
        cleanup: "cleanup",
		aliased_imports: "aliased-imports",
		child: "child",
		child_sync: "child-sync",
		suspending: "suspending",
		fuel_suspending: "fuel-suspending",
		ordered_root: "ordered-root",
		shared_leaf: "shared-leaf",
		suspending_child: "suspending-child",
		sync_import: "sync-import",
	};
}

#[derive( Debug, thiserror::Error )]
enum TestHostError {
	#[error( "host wait queue lock was poisoned" )] QueuePoisoned,
	#[error( "host received more wait calls than the test prepared" )] UnexpectedWait,
	#[error( "host wait release was dropped" )] ReleaseDropped,
}

fn test_engine() -> Result<Engine, wasmtime::Error> {
	let mut config = wasmtime::Config::new();
	config.wasm_component_model_implements( true );
	Engine::new( &config )
}

#[test]
fn all_import_aliases_are_linked_for_sync_and_async_sockets() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		for asynchronous in [ false, true ] {
			let plugins = fixtures::plugins( &engine );
			let bindings = fixtures::bindings();
			let linker = Linker::new( &engine );
			let socket: wasm_link::BindingAnyAsync<String, TestContext> = if asynchronous {
				Binding::new(
					bindings.dependency.package,
					HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
					ExactlyOne( "child".to_string(), plugins.child.plugin.instantiate_async( &engine, &linker ).await? ),
				).into()
			} else {
				Binding::new(
					bindings.dependency.package,
					HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
					ExactlyOne( "child".to_string(), plugins.child_sync.plugin.instantiate( &engine, &linker )? ),
				).into()
			};
			let instance = plugins.aliased_imports.plugin.link_async( &engine, linker, [ socket ]).await?;
			let root = Binding::new(
				bindings.root.package,
				HashMap::from([( bindings.root.name, bindings.root.spec )]),
				ExactlyOne( "root".to_string(), instance ),
			);
			assert_u32( &root.dispatch( "root", "get-primitive", &[] ).await, 42 );
		}
		Ok(())
	})
}

fn suspending_interface() -> Interface {
	Interface::new(
		HashMap::from([(
			"get-value".to_string(),
			Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ),
		)]),
		HashSet::new(),
	)
}

#[test]
fn opposite_dependency_orders_do_not_deadlock() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let calls = Arc::new( AtomicUsize::new( 0 ));
		let ( release_first, first_wait ) = futures::channel::oneshot::channel();
		let ( release_second, second_wait ) = futures::channel::oneshot::channel();
		let linker = suspending_linker( &engine, VecDeque::from([ first_wait, second_wait ]), calls.clone() )?;
		let left = fixtures::plugins( &engine ).shared_leaf.plugin.instantiate_async( &engine, &linker ).await?;
		let right = fixtures::plugins( &engine ).shared_leaf.plugin.instantiate_async( &engine, &linker ).await?;
		let left = Binding::new( "test:left", HashMap::from([( "root".to_string(), suspending_interface() )]), ExactlyOne( "left".to_string(), left ));
		let right = Binding::new( "test:right", HashMap::from([( "root".to_string(), suspending_interface() )]), ExactlyOne( "right".to_string(), right ));
		let first = fixtures::plugins( &engine ).ordered_root.plugin
			.link_async( &engine, linker.clone(), [ left.clone(), right.clone() ]).await?;
		let second = fixtures::plugins( &engine ).ordered_root.plugin
			.link_async( &engine, linker, [ left, right ]).await?;
		let interface = Interface::new(
			[ "forward", "reverse" ].into_iter().map(| name | (
				name.to_string(), Function::new( FunctionKind::Freestanding, ReturnKind::AssumeNoResources ),
			)).collect(), HashSet::new(),
		);
		let first = Binding::new( "test:ordered", HashMap::from([( "root".to_string(), interface.clone() )]), ExactlyOne( "first".to_string(), first ));
		let second = Binding::new( "test:ordered", HashMap::from([( "root".to_string(), interface )]), ExactlyOne( "second".to_string(), second ));
		let mut first = Box::pin( first.dispatch( "root", "forward", &[] ));
		let mut second = Box::pin( second.dispatch( "root", "reverse", &[] ));
		poll_until_calls( first.as_mut(), &calls, 1 );
		poll_until_calls( second.as_mut(), &calls, 2 );
		assert!( release_first.send(()).is_ok() );
		assert!( release_second.send(()).is_ok() );
		let mut both = Box::pin( futures::future::join( first, second ));
		let waker = futures::task::noop_waker();
		let mut context = Context::from_waker( &waker );
		for _ in 0..100 {
			if let Poll::Ready(( first, second )) = both.as_mut().poll( &mut context ) {
				assert_u32( &first, 42 );
				assert_u32( &second, 42 );
				return Ok(());
			}
		}
		Err( "shared leaf plugins deadlocked after both host waits were released".into() )
	})
}

#[test]
fn overlapping_calls_keep_independent_fuel_after_suspension() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let mut config = wasmtime::Config::new();
		config.wasm_component_model_implements( true ).consume_fuel( true );
		let engine = Engine::new( &config )?;
		let calls = Arc::new( AtomicUsize::new( 0 ));
		let ( release_first, first_wait ) = futures::channel::oneshot::channel();
		let ( release_second, second_wait ) = futures::channel::oneshot::channel();
		let linker = suspending_linker( &engine, VecDeque::from([ first_wait, second_wait ]), calls.clone() )?;
		let budgets = Arc::new( AtomicUsize::new( 0 ));
		let budget_calls = budgets.clone();
		let instance = fixtures::plugins( &engine ).fuel_suspending.plugin
			.with_initial_fuel( 100_000 )
			.with_fuel_limiter( move | _, _, _, _ | {
				if budget_calls.fetch_add( 1, Ordering::SeqCst ) == 0 { 1000 } else { 100_000 }
			})
			.instantiate_async( &engine, &linker ).await?;
		let binding = Binding::new(
			"test:plugin", HashMap::from([( "root".to_string(), suspending_interface() )]),
			ExactlyOne( "plugin".to_string(), instance ),
		);
		let mut first = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		let mut second = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		poll_until_calls( first.as_mut(), &calls, 1 );
		poll_until_calls( second.as_mut(), &calls, 2 );
		assert!( release_second.send(()).is_ok() );
		assert_u32( &second.await, 42 );
		assert!( release_first.send(()).is_ok() );
		let result = first.await;
		let Ok( ExactlyOne( _, Err( wasm_link::CallError::RuntimeException( error )))) = result else {
			return Err( format!( "first call borrowed the second call's fuel: {result:?}" ).into() );
		};
		assert!( format!( "{error:#}" ).contains( "fuel" ), "unexpected failure: {error:#}" );
		assert_eq!( budgets.load( Ordering::SeqCst ), 2 );
		Ok(())
	})
}

#[test]
fn fuel_limited_plugins_can_route_async_dependencies() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let mut config = wasmtime::Config::new();
		config.wasm_component_model_implements( true ).consume_fuel( true );
		let engine = Engine::new( &config )?;
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let child = plugins.child.plugin.with_initial_fuel( 100_000 )
			.with_fuel_limiter(| _, _, _, _ | 100_000 )
			.instantiate_async( &engine, &Linker::new( &engine )).await?;
		let dependency = Binding::new(
			bindings.dependency.package, HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "child".to_string(), child ),
		);
		let startup = plugins.startup.plugin.with_initial_fuel( 100_000 )
			.with_fuel_limiter(| _, _, _, _ | 100_000 )
			.link_async( &engine, Linker::new( &engine ), [ dependency ]).await?;
		let root = Binding::new(
			bindings.root.package, HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "root".to_string(), startup ),
		);
		assert_u32( &root.dispatch( "root", "get-primitive", &[] ).await, 42 );
		Ok(())
	})
}

#[test]
fn epoch_deadlines_include_dependency_waits() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		for expire in [ false, true ] {
			let mut config = wasmtime::Config::new();
			config.wasm_component_model_implements( true ).epoch_interruption( true );
			let engine = Engine::new( &config )?;
			let calls = Arc::new( AtomicUsize::new( 0 ));
			let ( release, wait ) = futures::channel::oneshot::channel();
			let linker = suspending_linker( &engine, VecDeque::from([ wait ]), calls.clone() )?;
			let plugins = fixtures::plugins( &engine );
			let bindings = fixtures::bindings();
			let child = plugins.suspending_child.plugin.with_epoch_limiter(| _, _, _, _ | 100_000 )
				.instantiate_async( &engine, &linker ).await?;
			let dependency = Binding::new(
				bindings.dependency.package, HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
				ExactlyOne( "child".to_string(), child ),
			);
			let parent = plugins.startup.plugin.with_epoch_limiter(| _, _, _, _ | 2 )
				.link_async( &engine, Linker::new( &engine ), [ dependency ]).await?;
			let root = Binding::new(
				bindings.root.package, HashMap::from([( bindings.root.name, bindings.root.spec )]),
				ExactlyOne( "root".to_string(), parent ),
			);
			let mut dispatch = Box::pin( root.dispatch( "root", "get-primitive", &[] ));
			poll_until_calls( dispatch.as_mut(), &calls, 1 );
			if expire { engine.increment_epoch(); engine.increment_epoch(); }
			assert!( release.send(()).is_ok() );
			let result = dispatch.await;
			if expire {
				let Ok( ExactlyOne( _, Err( wasm_link::CallError::RuntimeException( error )))) = result else {
					return Err( format!( "dependency wait did not count toward the epoch deadline: {result:?}" ).into() );
				};
				assert!( format!( "{error:#}" ).contains( "interrupt" ), "unexpected failure: {error:#}" );
			} else {
				assert_u32( &result, 42 );
			}
		}
		Ok(())
	})
}

#[test]
fn dropped_epoch_queued_call_is_not_executed_by_the_next_dispatch() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let calls = Arc::new( AtomicUsize::new( 0 ));
		let ( release_first, first_wait ) = futures::channel::oneshot::channel();
		let ( release_next, next_wait ) = futures::channel::oneshot::channel();
		let linker = suspending_linker( &engine, VecDeque::from([ first_wait, next_wait ]), calls.clone() )?;
		let instance = fixtures::plugins( &engine ).suspending.plugin
			.with_epoch_limiter(| _, _, _, _ | 1000 )
			.instantiate_async( &engine, &linker ).await?;
		let binding = Binding::new(
			"test:plugin", HashMap::from([( "root".to_string(), suspending_interface() )]),
			ExactlyOne( "plugin".to_string(), instance ),
		);
		let mut first = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		poll_until_calls( first.as_mut(), &calls, 1 );
		let mut cancelled = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		let waker = futures::task::noop_waker();
		let mut context = Context::from_waker( &waker );
		assert!( cancelled.as_mut().poll( &mut context ).is_pending() );
		assert_eq!( calls.load( Ordering::SeqCst ), 1 );
		drop( cancelled );
		assert!( release_first.send(()).is_ok() );
		assert_u32( &first.await, 42 );
		let mut next = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		poll_until_calls( next.as_mut(), &calls, 2 );
		assert!( release_next.send(()).is_ok() );
		assert_u32( &next.await, 42 );
		assert_eq!( calls.load( Ordering::SeqCst ), 2 );
		Ok(())
	})
}

type SingleDispatch = Result<ExactlyOne<String, Result<Val, wasm_link::CallError>>, wasm_link::DispatchError>;

fn assert_u32( result: &SingleDispatch, expected: u32 ) {
	assert!(
		matches!( &result, Ok( ExactlyOne( _, Ok( Val::U32( value )))) if *value == expected ),
		"unexpected dispatch result: {result:#?}",
	);
}

fn poll_until_calls<F: Future>( mut future: std::pin::Pin<&mut F>, calls: &AtomicUsize, expected: usize ) {
	let waker = futures::task::noop_waker();
	let mut context = Context::from_waker( &waker );
	for _ in 0..100 {
		assert!( matches!( future.as_mut().poll( &mut context ), Poll::Pending ));
		if calls.load( Ordering::SeqCst ) == expected { return; }
	}
	assert_eq!( calls.load( Ordering::SeqCst ), expected, "host wait call count" );
}

fn suspending_linker(
	engine: &Engine,
	waits: VecDeque<futures::channel::oneshot::Receiver<()>>,
	calls: Arc<AtomicUsize>,
) -> Result<Linker<TestContext>, wasmtime::Error> {
	let waits = Arc::new( StdMutex::new( waits ));
	let host_waits = Arc::clone( &waits );
	let mut linker = Linker::new( engine );
	linker.root().instance( "test:host/root" )?
		.func_new_concurrent( "wait", move | _, _, _, results | {
			calls.fetch_add( 1, Ordering::SeqCst );
			let wait = host_waits.lock()
				.map_err(| _ | TestHostError::QueuePoisoned )
				.and_then(| mut waits | waits.pop_front().ok_or( TestHostError::UnexpectedWait ));
			Box::pin( async move {
				wait?.await.map_err(| _ | TestHostError::ReleaseDropped )?;
				results[0] = Val::U32( 42 );
				Ok(())
			})
		})?;
	Ok( linker )
}

#[test]
fn link_async_accepts_a_sync_socket_binding() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let linker = Linker::new( &engine );
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let child = plugins.child_sync.plugin.instantiate( &engine, &linker )?;
		let dependency = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child ),
		);
		let startup = plugins.startup.plugin.link_async( &engine, linker, vec![ dependency ]).await?;
		let root = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), startup ),
		);
		assert_u32( &root.dispatch( "root", "get-primitive", &[] ).await, 42 );
		Ok(())
	})
}

#[test]
fn async_binding_accepts_a_sync_plugin_instance() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let linker = Linker::new( &engine );
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let child = plugins.child_sync.plugin.instantiate( &engine, &linker )?;
		let child: PluginInstanceAsync<TestContext> = child.into();
		let dependency = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child ),
		);
		let startup = plugins.startup.plugin.link_async( &engine, linker, vec![ dependency ]).await?;
		let root = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), startup ),
		);
		assert_u32( &root.dispatch( "root", "get-primitive", &[] ).await, 42 );
		Ok(())
	})
}

#[test]
fn a_shared_async_dependency_remains_dispatchable_across_sessions() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let child = plugins.child.plugin.instantiate_async( &engine, &Linker::new( &engine )).await?;
		let dependency = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child ),
		);
		let first = plugins.startup.plugin
			.link_async( &engine, Linker::new( &engine ), vec![ dependency.clone() ]).await?;
		let root = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), first ),
		);
		assert_u32( &root.dispatch( "root", "get-primitive", &[] ).await, 42 );

		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let second = plugins.startup.plugin
			.link_async( &engine, Linker::new( &engine ), vec![ dependency ]).await?;
		let root = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), second ),
		);
		assert_u32( &root.dispatch( "root", "get-primitive", &[] ).await, 42 );
		Ok(())
	})
}

#[test]
fn sync_import_accepts_an_async_export() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let child = plugins.child.plugin.instantiate_async( &engine, &Linker::new( &engine )).await?;
		let dependency = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child ),
		);
		let _ = plugins.sync_import.plugin
			.link_async( &engine, Linker::new( &engine ), vec![ dependency ]).await?;
		Ok(())
	})
}

#[test]
fn async_linking_does_not_infer_effects_from_sync_instance_exports() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();
		let child = plugins.child.plugin.instantiate( &engine, &Linker::new( &engine ))?;
		let dependency = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child ),
		);
		let _ = plugins.startup.plugin
			.link_async( &engine, Linker::new( &engine ), vec![ dependency ]).await?;
		Ok(())
	})
}

#[test]
fn independent_sessions_can_suspend_in_the_same_plugin() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let calls = Arc::new( AtomicUsize::new( 0 ));
		let ( release_first, first_wait ) = futures::channel::oneshot::channel();
		let ( release_second, second_wait ) = futures::channel::oneshot::channel();
		let linker = suspending_linker(
			&engine,
			VecDeque::from([ first_wait, second_wait ]),
			Arc::clone( &calls ),
		)?;
		let instance = fixtures::plugins( &engine ).suspending.plugin
			.instantiate_async( &engine, &linker ).await?;
		let binding = Binding::new(
			"test:plugin",
			HashMap::from([( "root".to_string(), suspending_interface() )]),
			ExactlyOne( "plugin".to_string(), instance ),
		);
		let mut first = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		let mut second = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		poll_until_calls( first.as_mut(), &calls, 1 );
		let waker = futures::task::noop_waker();
		let mut context = Context::from_waker( &waker );
		assert!( second.as_mut().poll( &mut context ).is_pending() );
		poll_until_calls( second.as_mut(), &calls, 2 );
		assert!( release_second.send(()).is_ok(), "second host wait was dropped" );
		assert_u32( &second.await, 42 );
		assert!( release_first.send(()).is_ok(), "first host wait was dropped" );
		assert_u32( &first.await, 42 );
		Ok(())
	})
}

#[test]
fn dropping_a_dispatch_releases_the_graph_for_the_next_session() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let calls = Arc::new( AtomicUsize::new( 0 ));
		let ( release_first, first ) = futures::channel::oneshot::channel();
		let ( release_second, second ) = futures::channel::oneshot::channel();
		let linker = suspending_linker(
			&engine,
			VecDeque::from([ first, second ]),
			Arc::clone( &calls ),
		)?;
		let instance = fixtures::plugins( &engine ).suspending.plugin
			.instantiate_async( &engine, &linker ).await?;
		let binding = Binding::new(
			"test:plugin",
			HashMap::from([( "root".to_string(), suspending_interface() )]),
			ExactlyOne( "plugin".to_string(), instance ),
		);

		let mut first = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		poll_until_calls( first.as_mut(), &calls, 1 );
		drop( first );
		assert!( release_first.send(()).is_ok(), "first host wait was dropped" );

		let mut second = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		poll_until_calls( second.as_mut(), &calls, 2 );
		assert!( release_second.send(()).is_ok(), "second host wait was dropped" );
		assert_u32( &second.await, 42 );
		Ok(())
	})
}

#[test]
fn one_session_keeps_multiple_plugin_calls_suspended() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = test_engine()?;
		let calls = Arc::new( AtomicUsize::new( 0 ));
		let ( release_first, first ) = futures::channel::oneshot::channel();
		let ( release_second, second ) = futures::channel::oneshot::channel();
		let linker = suspending_linker(
			&engine,
			VecDeque::from([ first, second ]),
			Arc::clone( &calls ),
		)?;
		let first_instance = fixtures::plugins( &engine ).suspending.plugin
			.instantiate_async( &engine, &linker ).await?;
		let second_instance = fixtures::plugins( &engine ).suspending.plugin
			.instantiate_async( &engine, &linker ).await?;
		let binding = Binding::new(
			"test:plugin",
			HashMap::from([( "root".to_string(), suspending_interface() )]),
			Any( HashMap::from([
				( "first".to_string(), first_instance ),
				( "second".to_string(), second_instance ),
			])),
		);

		let mut dispatch = Box::pin( binding.dispatch( "root", "get-value", &[] ));
		poll_until_calls( dispatch.as_mut(), &calls, 2 );
		assert!( release_first.send(()).is_ok(), "first host wait was dropped" );
		assert!( release_second.send(()).is_ok(), "second host wait was dropped" );
		let Any( results ) = dispatch.await?;
		assert_eq!( results.len(), 2 );
		assert!( results.values().all(| value | matches!( value, Ok( Val::U32( 42 )))));
		Ok(())
	})
}

#[test]
fn dropped_call_observes_cancellation_when_another_call_drives_the_store() -> Result<(), Box<dyn std::error::Error>> {
    futures::executor::block_on(async {
        let engine = test_engine()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let observations = Arc::new(StdMutex::new(Vec::<(u64, bool)>::new()));
        let recorded = observations.clone();
        let (release_first, first_wait) = futures::channel::oneshot::channel();
        let (release_second, second_wait) = futures::channel::oneshot::channel();
        let mut linker = suspending_linker(&engine, VecDeque::from([first_wait, second_wait]), calls.clone())?;
        linker.root().instance("test:observe/root")?.func_wrap("record", move |_, values: (u64, bool)| {
            recorded.lock().map_err(|_| TestHostError::QueuePoisoned)?.push(values);
            Ok(())
        })?;
        let instance = fixtures::plugins(&engine).cancellation.plugin.instantiate_async(&engine, &linker).await?;
        let binding = Binding::new("test:plugin", HashMap::from([("root".into(), suspending_interface())]), ExactlyOne("plugin".to_string(), instance));
        let mut first = Box::pin(binding.dispatch("root", "get-value", &[]));
        poll_until_calls(first.as_mut(), &calls, 1);
        drop(first);
        assert!(release_first.send(()).is_ok());
        let mut second = Box::pin(binding.dispatch("root", "get-value", &[]));
        poll_until_calls(second.as_mut(), &calls, 2);
        assert!(release_second.send(()).is_ok());
        assert_u32(&second.await, 42);
        let observed = observations.lock().map_err(|_| TestHostError::QueuePoisoned)?;
        assert_eq!(observed.len(), 2);
        assert!(observed[0].1);
        assert!(!observed[1].1);
        assert_ne!(observed[0].0, observed[1].0);
        Ok(())
    })
}

#[test]
fn async_entry_waits_through_an_already_linked_sync_dependency_tree() -> Result<(), Box<dyn std::error::Error>> {
    futures::executor::block_on(async {
        let engine = test_engine()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let (release_first, first_wait) = futures::channel::oneshot::channel();
        let (release_second, second_wait) = futures::channel::oneshot::channel();
        let linker = suspending_linker(&engine, VecDeque::from([first_wait, second_wait]), calls.clone())?;
        let plugins = fixtures::plugins(&engine);
        let bindings = fixtures::bindings();
        let child = plugins.suspending_child.plugin.instantiate(&engine, &linker)?;
        let dependency = Binding::new(bindings.dependency.package, HashMap::from([(bindings.dependency.name, bindings.dependency.spec)]), ExactlyOne("child".to_string(), child));
        // Link the whole dependency tree synchronously before making async entry possible.
        let parent = plugins.startup.plugin.link(&engine, linker, vec![dependency.clone()])?;
        let root = Binding::new(bindings.root.package, HashMap::from([(bindings.root.name, bindings.root.spec)]), ExactlyOne("root".to_string(), PluginInstanceAsync::from(parent)));
        let worker_binding = dependency.clone();
        let worker = std::thread::spawn(move || worker_binding.dispatch("root", "get-value", &[]));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while calls.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline { std::thread::yield_now(); }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(matches!(dependency.dispatch("root", "get-value", &[]), Ok(ExactlyOne(_, Err(wasm_link::SyncCallError::PluginBusy)))));
        let mut dispatch = Box::pin(root.dispatch("root", "get-primitive", &[]));
        let waker = futures::task::noop_waker();
        assert!(dispatch.as_mut().poll(&mut Context::from_waker(&waker)).is_pending());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(release_first.send(()).is_ok());
        let first = worker.join().map_err(|_| "sync worker panicked")??;
        assert!(matches!(first, ExactlyOne(_, Ok(Val::U32(42)))));
        poll_until_calls(dispatch.as_mut(), &calls, 2);
        assert!(release_second.send(()).is_ok());
        assert_u32(&dispatch.await, 42);
        Ok(())
    })
}

#[test]
fn abandoned_fanout_cleans_completed_and_later_nested_resource_results() -> Result<(), Box<dyn std::error::Error>> {
    futures::executor::block_on(async {
        let engine = test_engine()?;
        let drops = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let (release, wait) = futures::channel::oneshot::channel();
        let pending = Arc::new(StdMutex::new(Some(wait)));
        let mut instances = HashMap::new();
        for id in ["immediate", "pending"] {
            let mut linker = Linker::<TestContext>::new(&engine);
            let drop_count = drops.clone();
            linker.root().instance("test:cleanup/root")?.func_wrap("dropped", move |_, (_rep,): (u32,)| { drop_count.fetch_add(1, Ordering::SeqCst); Ok(()) })?;
            let pending = pending.clone();
            let started = started.clone();
            linker.root().instance("test:cleanup/root")?.func_wrap_async("pause", move |_, (): ()| {
                let wait = if id == "pending" { pending.lock().map_err(|_| TestHostError::QueuePoisoned).map(|mut value| value.take()) } else { Ok(None) };
                started.fetch_add(1, Ordering::SeqCst);
                Box::new(async move { if let Some(wait) = wait? { wait.await.map_err(|_| TestHostError::ReleaseDropped)?; } Ok(()) })
            })?;
            instances.insert(id.to_string(), fixtures::plugins(&engine).cleanup.plugin.instantiate_async(&engine, &linker).await?);
        }
        let interface = Interface::new(HashMap::from([
            ("make".into(), Function::new(FunctionKind::Freestanding, ReturnKind::MayContainResources)),
            ("ping".into(), Function::new(FunctionKind::Freestanding, ReturnKind::AssumeNoResources)),
        ]), HashSet::from(["item".into()]));
        let binding = Binding::new("test:cleanup-plugin", HashMap::from([("root".into(), interface)]), Any(instances));
        let mut make = Box::pin(binding.dispatch("root", "make", &[]));
        poll_until_calls(make.as_mut(), &started, 2);
        drop(make);
        assert!(release.send(()).is_ok());
        for _ in 0..3 { let Any(results) = binding.dispatch("root", "ping", &[]).await?; assert!(results.values().all(Result::is_ok), "{results:?}"); }
        assert_eq!(drops.load(Ordering::SeqCst), 4);
        let Any(results) = binding.dispatch("root", "make", &[]).await?;
        assert_eq!(drops.load(Ordering::SeqCst), 4, "delivered resources must stay alive");
        for (id, result) in results {
            let value = result?;
            let error = binding.release_resources(&"unknown".to_string(), value).err().ok_or("unknown owner accepted")?;
            binding.release_resources(&id, error.value)?;
        }
        let Any(results) = binding.dispatch("root", "ping", &[]).await?;
        assert!(results.values().all(Result::is_ok), "{results:?}");
        assert_eq!(drops.load(Ordering::SeqCst), 8);
        Ok(())
    })
}
