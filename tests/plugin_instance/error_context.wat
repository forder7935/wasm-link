(component
	(core module $memory
		(memory (export "memory") 1)
		(data (i32.const 0) "error context")
	)
	(core instance $memory (instantiate $memory))
	(core func $error-context-new
		(canon error-context.new (memory $memory "memory"))
	)

	(core module $implementation
		(import "" "error-context.new" (func $error-context-new (param i32 i32) (result i32)))
		(func (export "make-error-context") (result i32)
			i32.const 0
			i32.const 13
			call $error-context-new
		)
	)
	(core instance $implementation (instantiate $implementation
		(with "" (instance
			(export "error-context.new" (func $error-context-new))
		))
	))
	(func (export "make-error-context") async (result error-context)
		(canon lift (core func $implementation "make-error-context"))
	)
)
