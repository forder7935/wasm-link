(component
	(import "test:child/root" (instance $child
		(export "get-value" (func (result (tuple string (result u32)))))
	))
)
