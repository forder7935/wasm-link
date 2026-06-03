(component
	(core module $m
		(func $get_value (export "legacy-get-value") (result i32)
			i32.const 43
		)
	)
	(core instance $i (instantiate $m))
	(func $f (export "legacy-get-value") (result u32) (canon lift (core func $i "legacy-get-value")))
	(instance $inst
		(export "legacy-get-value" (func $f))
	)
	(export "test:remap-combined/remapped" (instance $inst))
)
