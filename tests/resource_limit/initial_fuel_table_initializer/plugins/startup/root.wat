(component
	(core module $m
		(func $f)
		(table 1 funcref)
		(elem (i32.add (i32.const 0) (i32.const 0)) func $f)
	)
	(core instance (instantiate $m))
)
