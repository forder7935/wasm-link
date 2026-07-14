use std::collections::HashMap;
use wasm_link::{ Binding, Engine, Linker, Val };
use wasm_link::cardinality::ExactlyOne ;

fixtures! {
	bindings = { root: "root", dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

#[test]
fn dispatches_wit_async_across_plugin_stores() {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let plugins = fixtures::plugins( &engine );
		let bindings = fixtures::bindings();

		let child_instance = plugins.child.plugin
			.instantiate_async( &engine, &linker )
			.await
			.expect( "Failed to instantiate child plugin asynchronously" );
		let dependency_binding = Binding::new(
			bindings.dependency.package,
			HashMap::from([( bindings.dependency.name, bindings.dependency.spec )]),
			ExactlyOne( "_".to_string(), child_instance ),
		);

		let startup_instance = plugins.startup.plugin
			.link_async( &engine, linker, vec![ dependency_binding ])
			.await
			.expect( "Failed to link startup plugin asynchronously" );
		let root_binding = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "_".to_string(), startup_instance ),
		);

		match root_binding.dispatch( "root", "get-primitive", &[] ) {
			Ok( ExactlyOne( _, Err( wasm_link::DispatchError::AsyncRequired ))) => {}
			value => panic!( "Expected sync dispatch to require async dispatch, found: {:#?}", value ),
		}

		match root_binding.dispatch_async( "root", "get-primitive", &[] ).await {
			Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
			value => panic!( "Expected Ok( ExactlyOne( Ok( U32( 42 )))), found: {:#?}", value ),
		}

		let ( first, second ) = futures::join!(
			root_binding.dispatch_async( "root", "get-primitive", &[] ),
			root_binding.dispatch_async( "root", "get-primitive", &[] ),
		);
		for value in [ first, second ] {
			match value {
				Ok( ExactlyOne( _, Ok( Val::U32( 42 )))) => {}
				value => panic!( "Expected queued async dispatch to return U32(42), found: {:#?}", value ),
			}
		}
	});
}
