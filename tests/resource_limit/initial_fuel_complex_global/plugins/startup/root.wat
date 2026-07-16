(component
	(core module $m
		(global i32 (i32.add (i32.const 1) (i32.const 2)))
	)
	(core instance (instantiate $m))
)
