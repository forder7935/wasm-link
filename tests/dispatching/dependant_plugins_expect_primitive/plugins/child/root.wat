(component
	(core module $m
		(func $get_value (export "get-value") (result i32)
			(local $iterations i32)
			(loop $delay
				local.get $iterations
				i32.const 1
				i32.add
				local.tee $iterations
				i32.const 10000000
				i32.lt_u
				br_if $delay
			)
			i32.const 42
		)
	)
	(core instance $i (instantiate $m))
	(func $f (export "get-value") (result u32) (canon lift (core func $i "get-value")))
	(instance $inst
		(export "get-value" (func $f))
	)
		(export "test:child/root" (instance $inst))
)
