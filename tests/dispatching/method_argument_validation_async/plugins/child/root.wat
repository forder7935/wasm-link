(component
	(core module $m
		(func (export "get-value") (result i32) i32.const 42)
	)
	(core instance $i (instantiate $m))
	(func $f async (result u32) (canon lift (core func $i "get-value")))
	(instance $inst (export "get-value" (func $f)))
	(export "test:async-validation-child/root" (instance $inst))
)
