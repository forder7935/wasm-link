(component
	(core module $m
		(func (export "run"))
	)
	(core instance $i (instantiate $m))
	(func $run (canon lift (core func $i "run")))
	(instance $root (export "run" (func $run)))
	(export "test:void/root" (instance $root))
)
