(component
	;; Import level-b dependency
	(import "test:level-b/root" (instance $level_b
		(export "get-b" (func (result (tuple string (result u32)))))
	))

	(alias export $level_b "get-b" (func $get_b))

	;; Memory for lowering
	(core module $mem_module
		(memory (export "memory") 1)
		(func (export "realloc") (param i32 i32 i32 i32) (result i32)
			i32.const 256
		)
	)
	(core instance $mem_inst (instantiate $mem_module))
	(alias core export $mem_inst "memory" (core memory $shared_mem))
	(alias core export $mem_inst "realloc" (core func $shared_realloc))

	(core func $lowered_get_b (canon lower (func $get_b) (memory $shared_mem) (realloc $shared_realloc)))
	(core instance $imports_b (export "get-b" (func $lowered_get_b)))
	(core instance $mem_imports (export "memory" (memory $shared_mem)))

	(core module $main_impl
		(import "level-b" "get-b" (func $get_b (param i32)))
		(import "mem" "memory" (memory 1))

		(func (export "get-value") (result i32)
			(call $get_b (i32.const 0))
			(i32.load (i32.const 12))
		)
	)

	(core instance $main_inst (instantiate $main_impl
		(with "level-b" (instance $imports_b))
		(with "mem" (instance $mem_imports))
	))

	(alias core export $main_inst "get-value" (core func $core_get_value))
	(func $lifted_get_value (result u32) (canon lift (core func $core_get_value)))
	(instance $inst (export "get-value" (func $lifted_get_value)))
	(export "test:topology/root" (instance $inst))
)
