use std::collections::{ HashMap, HashSet };

use wasm_link::cardinality::ExactlyOne;
use wasm_link::concurrent::{Binding, Function, Interface};
use wasm_link::{Engine, FunctionKind, Linker, ReturnKind, Val};

fixtures! {
	bindings = { root: "root", dependency: "dependency" };
	plugins  = { startup: "startup", child: "child" };
}

#[test]
fn native_async_method_metadata_rejects_calls_without_resource_argument() -> Result<(), Box<dyn std::error::Error>> {
	futures::executor::block_on( async {
		let engine = Engine::default();
		let linker = Linker::new( &engine );
		let executor = futures::executor::ThreadPool::new()?;
        let plugins = fixtures::plugins_concurrent(&engine);
        let bindings = fixtures::bindings_concurrent();
        let child = plugins
            .child
            .plugin
            .instantiate(&engine, &linker, executor.clone())
            .await?;
		let dependency = Binding::new(
			bindings.dependency.package,
			HashMap::from([(
				bindings.dependency.name,
				Interface::new(
					HashMap::from([(
						"get-value".to_string(),
						Function::new_async( FunctionKind::Method, ReturnKind::AssumeNoResources ),
					)]),
					HashSet::new(),
				),
			)]),
			ExactlyOne( "child".to_string(), child ),
		);
        let startup = plugins
            .startup
            .plugin
            .link(&engine, linker, vec![dependency], executor)
            .await?;
		let root = Binding::new(
			bindings.root.package,
			HashMap::from([( bindings.root.name, bindings.root.spec )]),
			ExactlyOne( "startup".to_string(), startup ),
		);

        let result = root.dispatch("root", "get-primitive", &[]).await?;
        assert!(
            matches!(result, ExactlyOne(_, Ok(Val::Result(Err(None))))),
            "unexpected dispatch result: {result:#?}"
        );
		Ok(())
	})
}
