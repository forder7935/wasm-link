(component
	(core module $m
		(func $test (export "test") (result i32)
			i32.const 42
		)
	)
	(core instance $i (instantiate $m))
	(func $f (export "test") (result u32) (canon lift (core func $i "test")))
	(type $not-a-function (record (field "value" u32)))
	(instance $inst
		(export "test" (func $f))
		(export "not-a-function" (type $not-a-function))
	)
	(export "test:dispatch-error/root" (instance $inst))
)
