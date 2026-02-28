(component
	;; Import the three dependency interfaces
	;; Each function returns tuple<string, result<u32>> because cross-plugin calls are wrapped
	(import "test:dependency-one/root" (instance $dependency_one
		(export "get-one" (func (result (tuple string (result u32)))))
	))
	(import "test:dependency-two/root" (instance $dependency_two
		(export "get-two" (func (result (tuple string (result u32)))))
	))
	(import "test:dependency-three/root" (instance $dependency_three
		(export "get-three" (func (result (tuple string (result u32)))))
	))

	;; Alias the imported functions
	(alias export $dependency_one "get-one" (func $get_one))
	(alias export $dependency_two "get-two" (func $get_two))
	(alias export $dependency_three "get-three" (func $get_three))

	;; Memory provider module (needed for lowering functions that return results)
	(core module $mem_module
		(memory (export "memory") 1)
		(func (export "realloc") (param i32 i32 i32 i32) (result i32)
			i32.const 256
		)
	)
	(core instance $mem_inst (instantiate $mem_module))
	(alias core export $mem_inst "memory" (core memory $shared_mem))
	(alias core export $mem_inst "realloc" (core func $shared_realloc))

	;; Lower the imported functions - each takes a retptr and writes discriminant+value
	(core func $lowered_get_one (canon lower (func $get_one) (memory $shared_mem) (realloc $shared_realloc)))
	(core func $lowered_get_two (canon lower (func $get_two) (memory $shared_mem) (realloc $shared_realloc)))
	(core func $lowered_get_three (canon lower (func $get_three) (memory $shared_mem) (realloc $shared_realloc)))

	;; Create import instances for the main module
	(core instance $imports_one (export "get-one" (func $lowered_get_one)))
	(core instance $imports_two (export "get-two" (func $lowered_get_two)))
	(core instance $imports_three (export "get-three" (func $lowered_get_three)))
	(core instance $mem_imports (export "memory" (memory $shared_mem)))

	;; Main implementation module
	(core module $main_impl
		(import "dependency-one" "get-one" (func $get_one (param i32)))
		(import "dependency-two" "get-two" (func $get_two (param i32)))
		(import "dependency-three" "get-three" (func $get_three (param i32)))
		(import "mem" "memory" (memory 1))

		;; get-value: calls all three dependencies and returns the sum (1+2+3=6)
		;; Memory layout: each tuple takes 16 bytes (8 for string, 8 for result<u32>)
		;; offset 0: tuple from get-one
		;; offset 16: tuple from get-two
		;; offset 32: tuple from get-three
		(func (export "get-value") (result i32)
			;; Call get-one with retptr=0
			(call $get_one (i32.const 0))
			;; Call get-two with retptr=16
			(call $get_two (i32.const 16))
			;; Call get-three with retptr=32
			(call $get_three (i32.const 32))

			;; Sum the values (at offsets 12, 28, 44 - after the string fields + result discriminants)
			(i32.add
				(i32.add
					(i32.load (i32.const 12))
					(i32.load (i32.const 28))
				)
				(i32.load (i32.const 44))
			)
		)
	)

	;; Instantiate main module with imports
	(core instance $main_inst (instantiate $main_impl
		(with "dependency-one" (instance $imports_one))
		(with "dependency-two" (instance $imports_two))
		(with "dependency-three" (instance $imports_three))
		(with "mem" (instance $mem_imports))
	))

	;; Lift and export
	(alias core export $main_inst "get-value" (core func $core_get_value))
	(func $lifted_get_value (result u32) (canon lift (core func $core_get_value)))
	(instance $inst (export "get-value" (func $lifted_get_value)))
	(export "test:multiple-sockets/root" (instance $inst))
)
