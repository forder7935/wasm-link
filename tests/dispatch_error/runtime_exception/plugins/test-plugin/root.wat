(component
	(core module $m
		(func $trap (export "trap")
			unreachable
		)
	)
	(core instance $i (instantiate $m))
	(func $f (export "trap") (canon lift (core func $i "trap")))
	(instance $inst
		(export "trap" (func $f))
	)
	(export "test:dispatch-error/root" (instance $inst))
)
