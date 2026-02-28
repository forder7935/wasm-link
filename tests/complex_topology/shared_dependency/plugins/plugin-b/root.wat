(component
  ;; Import binding-d dependency
  (import "test:binding-d/root" (instance $interface_d
    (export "get-d" (func (result (tuple string (result u32)))))
  ))

  (alias export $interface_d "get-d" (func $get_d))

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

  (core func $lowered_get_d (canon lower (func $get_d) (memory $shared_mem) (realloc $shared_realloc)))
  (core instance $imports_d (export "get-d" (func $lowered_get_d)))
  (core instance $mem_imports (export "memory" (memory $shared_mem)))

  (core module $main_impl
    (import "binding-d" "get-d" (func $get_d (param i32)))
    (import "mem" "memory" (memory 1))

    (func (export "get-b") (result i32)
      (call $get_d (i32.const 0))
      (i32.load (i32.const 12))
    )
  )

  (core instance $main_inst (instantiate $main_impl
    (with "binding-d" (instance $imports_d))
    (with "mem" (instance $mem_imports))
  ))

  (alias core export $main_inst "get-b" (core func $core_get_b))
  (func $lifted_get_b (result u32) (canon lift (core func $core_get_b)))
  (instance $inst (export "get-b" (func $lifted_get_b)))
  (export "test:binding-b/root" (instance $inst))
)
