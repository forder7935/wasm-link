(component
	(import "test:async-resource/root" (instance $resource_inst
		(export "counter" (type $counter (sub resource)))
		(export "make-counter" (func async (result (tuple string (result (own $counter))))))
		(export "[method]counter.get-value" (func async (param "self" (borrow $counter)) (result (result u32))))
	))
	(alias export $resource_inst "counter" (type $counter))
	(alias export $resource_inst "make-counter" (func $make_counter_wrapped))
	(alias export $resource_inst "[method]counter.get-value" (func $get_wrapped))

	(core module $mem_module
		(memory (export "memory") 1)
		(func (export "realloc") (param i32 i32 i32 i32) (result i32)
			i32.const 256
		)
	)
	(core instance $mem_inst (instantiate $mem_module))
	(alias core export $mem_inst "memory" (core memory $shared_mem))
	(alias core export $mem_inst "realloc" (core func $shared_realloc))
	(core func $lowered_make_counter
		(canon lower (func $make_counter_wrapped) (memory $shared_mem) (realloc $shared_realloc))
	)
	(core func $lowered_get
		(canon lower (func $get_wrapped) (memory $shared_mem) (realloc $shared_realloc))
	)
	(core instance $resource_imports
		(export "make-counter" (func $lowered_make_counter))
		(export "get" (func $lowered_get))
	)
	(core instance $mem_imports (export "memory" (memory $shared_mem)))

	(core module $main_impl
		(import "resource" "make-counter" (func $make_counter (param i32)))
		(import "resource" "get" (func $get (param i32 i32)))
		(import "mem" "memory" (memory 1))
		(func (export "get-value") (result i32)
			(call $make_counter (i32.const 0))
			(call $get (i32.load (i32.const 12)) (i32.const 16))
			(i32.load (i32.const 20))
		)
	)
	(core instance $main_inst (instantiate $main_impl
		(with "resource" (instance $resource_imports))
		(with "mem" (instance $mem_imports))
	))
	(alias core export $main_inst "get-value" (core func $core_get_value))
	(func $lifted_get_value async (result u32)
		(canon lift (core func $core_get_value))
	)
	(instance $consumer_inst (export "get-value" (func $lifted_get_value)))
	(export "test:async-consumer/root" (instance $consumer_inst))
)
