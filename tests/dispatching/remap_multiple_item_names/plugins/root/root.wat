(component
	(core module $m
		(func $get_one (export "legacy-get-one") (result i32)
			i32.const 1
		)
		(func $get_two (export "legacy-get-two") (result i32)
			i32.const 2
		)
	)
	(core instance $i (instantiate $m))
	(func $one (export "legacy-get-one") (result u32) (canon lift (core func $i "legacy-get-one")))
	(func $two (export "legacy-get-two") (result u32) (canon lift (core func $i "legacy-get-two")))
	(instance $inst
		(export "legacy-get-one" (func $one))
		(export "legacy-get-two" (func $two))
	)
	(export "test:remap-item-table/root" (instance $inst))
)
