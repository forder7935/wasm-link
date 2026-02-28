(component
	(core module $m
		(func $add (export "add") (param i32 i32) (result i32)
			local.get 0
			local.get 1
			i32.add
		)
	)
	(core instance $i (instantiate $m))
	(func $f (export "add") (param "a" u32) (param "b" u32) (result u32) (canon lift (core func $i "add")))
	(instance $inst
		(export "add" (func $f))
	)
	(export "test:dispatch-error/root" (instance $inst))
)
