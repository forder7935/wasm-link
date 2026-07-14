(component
	(core module $m
		(func (export "get-value") (result i32) i32.const 42)
	)
	(core instance $i (instantiate $m))
	(func $get-value (result u32) (canon lift (core func $i "get-value")))
	(instance $root (export "get-value" (func $get-value)))
	(export "test:child/root" (instance $root))
)
