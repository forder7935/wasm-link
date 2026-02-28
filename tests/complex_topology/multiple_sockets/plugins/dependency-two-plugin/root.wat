(component
	(core module $m
		(func $get_two (export "get-two") (result i32)
			i32.const 2
		)
	)
	(core instance $i (instantiate $m))
	(func $f (export "get-two") (result u32) (canon lift (core func $i "get-two")))
	(instance $inst
		(export "get-two" (func $f))
	)
	(export "test:dependency-two/root" (instance $inst))
)
