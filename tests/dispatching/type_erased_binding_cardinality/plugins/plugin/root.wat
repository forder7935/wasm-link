(component
	(core module $m
		(func (export "get-value") (result i32) i32.const 40)
	)
	(core instance $i (instantiate $m))
	(func $f (result u32) (canon lift (core func $i "get-value")))
	(instance $root (export "get-value" (func $f)))
	(export "test:remap-mixed/root" (instance $root))
)
