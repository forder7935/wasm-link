(component
	(type $child-interface (instance
		(type $dispatch-error' (variant
			(case "plugin-busy")
			(case "invalid-interface-path" string)
			(case "invalid-function" string)
			(case "runtime-exception" string)
			(case "invalid-argument-list")
			(case "unsupported-type" string)
			(case "resource-table-full")
			(case "resource-handle-conversion-failed")
			(case "invalid-resource-handle")
		))
		(export "dispatch-error" (type (eq $dispatch-error')))
		(type $dispatch-result (result u32 (error 1)))
		(type $get-value (func async (result (tuple string $dispatch-result))))
		(export "get-value" (func (type $get-value)))
	))
	(import "test:left/root" (instance $child (type $child-interface)))
	(import "test:right/root" (instance $right (type $child-interface)))
	(type $host-interface (instance (export "wait" (func async (result u32)))))
	(import "test:host/root" (instance $host (type $host-interface)))
	(alias export $right "get-value" (func $right_value))
	(alias export $host "wait" (func $wait))

	(alias export $child "get-value" (func $get_value))

	(core module $mem_module
		(memory (export "memory") 1)
		(func (export "realloc") (param i32 i32 i32 i32) (result i32)
			i32.const 256
		)
	)
	(core instance $mem_inst (instantiate $mem_module))
	(alias core export $mem_inst "memory" (core memory $shared_mem))
	(alias core export $mem_inst "realloc" (core func $shared_realloc))

	(core func $lowered_get_value
		(canon lower (func $get_value) (memory $shared_mem) (realloc $shared_realloc))
	)
	(core func $lowered_right (canon lower (func $right_value) (memory $shared_mem) (realloc $shared_realloc)))
	(core func $lowered_wait (canon lower (func $wait)))
	(core instance $imports_child
		(export "get-value" (func $lowered_get_value))
		(export "right" (func $lowered_right))
		(export "wait" (func $lowered_wait))
	)
	(core instance $mem_imports (export "memory" (memory $shared_mem)))

	(core module $main_impl
		(import "child" "get-value" (func $get_value (param i32)))
		(import "mem" "memory" (memory 1))

		(import "child" "right" (func $right (param i32)))
		(import "child" "wait" (func $wait (result i32)))
		(func (export "forward") (result i32)
			(call $get_value (i32.const 0))
			(drop (call $wait))
			(call $right (i32.const 0))
			(i32.load (i32.const 12))
		)
		(func (export "reverse") (result i32)
			(call $right (i32.const 0))
			(drop (call $wait))
			(call $get_value (i32.const 0))
			(i32.load (i32.const 12))
		)
	)

	(core instance $main_inst (instantiate $main_impl
		(with "child" (instance $imports_child))
		(with "mem" (instance $mem_imports))
	))

	(func $forward async (result u32) (canon lift (core func $main_inst "forward")))
	(func $reverse async (result u32) (canon lift (core func $main_inst "reverse")))
	(instance $root (export "forward" (func $forward)) (export "reverse" (func $reverse)))
	(export "test:ordered/root" (instance $root))
)
