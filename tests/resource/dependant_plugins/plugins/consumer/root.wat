(component
	;; Import the resource interface from the counter plugin
	;; When calling across plugin boundaries, results are wrapped in result<T, error>
	;; Using unit for error type to simplify (we're ignoring errors for this test)
	(import "test:myresource/root" (instance $resource_inst
		(export "counter" (type $counter (sub resource)))
		(export "make-counter" (func (result (tuple string (result (own $counter))))))
		(export "[method]counter.get-value" (func (param "self" (borrow $counter)) (result (result u32))))
	))

	;; Alias the imported types and functions
	(alias export $resource_inst "counter" (type $counter))
	(alias export $resource_inst "make-counter" (func $make_counter_wrapped))
	(alias export $resource_inst "[method]counter.get-value" (func $get_wrapped))

	;; Memory provider module
	(core module $mem_module
		(memory (export "memory") 1)
		(func (export "realloc") (param i32 i32 i32 i32) (result i32)
			i32.const 256
		)
	)
	(core instance $mem_inst (instantiate $mem_module))
	(alias core export $mem_inst "memory" (core memory $shared_mem))
	(alias core export $mem_inst "realloc" (core func $shared_realloc))

	;; Lower the imported functions using shared memory
	(core func $lowered_make_counter (canon lower (func $make_counter_wrapped) (memory $shared_mem) (realloc $shared_realloc)))
	(core func $lowered_get (canon lower (func $get_wrapped) (memory $shared_mem) (realloc $shared_realloc)))

	;; Create instance for imports
	(core instance $resource_imports
		(export "make-counter" (func $lowered_make_counter))
		(export "get" (func $lowered_get))
	)

	;; Main module that imports the shared memory
	(core module $main_impl
		(import "resource" "make-counter" (func $make_counter (param i32)))
		(import "resource" "get" (func $get (param i32 i32)))
		(import "mem" "memory" (memory 1))

		;; Our exported get-value function
		(func (export "get-value") (result i32)
			;; Call make-counter with retptr = 0
			i32.const 0
			call $make_counter
			
			;; Load handle from offset 12
			;; Call get-value with handle and retptr = 16
			(call $get
				(i32.load (i32.const 12))
				(i32.const 16)
			)
			
			;; Return the value at offset 20
			(i32.load (i32.const 20))
		)
	)

	;; Memory imports instance
	(core instance $mem_imports
		(export "memory" (memory $shared_mem))
	)

	;; Instantiate main module with proper imports
	(core instance $main_inst (instantiate $main_impl
		(with "resource" (instance $resource_imports))
		(with "mem" (instance $mem_imports))
	))

	;; Alias core export
	(alias core export $main_inst "get-value" (core func $core_get_value))

	;; Lift the get-value function
	(func $lifted_get_value (result u32)
		(canon lift (core func $core_get_value))
	)

	;; Export the consumer interface
	(instance $consumer_inst
		(export "get-value" (func $lifted_get_value))
	)
	(export "test:consumer/root" (instance $consumer_inst))
)
